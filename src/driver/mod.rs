use std::any::Any;
use std::cell::RefCell;
use std::mem;
use std::rc::Rc;
use std::task::Waker;

use io_uring::squeue::Entry;
use io_uring::{cqueue, IoUring};
use scoped_tls::scoped_thread_local;
use slab::Slab;

mod action;
mod op;

pub(crate) use action::{Action, Completable, CqeResult};
pub(crate) use op::*;

scoped_thread_local!(static CURRENT: Driver);

pub(crate) struct Driver {
    inner: Rc<RefCell<Inner>>,
}

impl Clone for Driver {
    fn clone(&self) -> Self {
        Driver {
            inner: self.inner.clone(),
        }
    }
}

struct Inner {
    ring: IoUring,
    actions: Slab<State>,
}

impl Driver {
    pub(crate) fn new() -> std::io::Result<Driver> {
        let ring = IoUring::new(256)?;
        let driver = Driver {
            inner: Rc::new(RefCell::new(Inner {
                ring,
                actions: Slab::with_capacity(256),
            })),
        };
        Ok(driver)
    }

    pub(crate) fn wait(&self) -> std::io::Result<()> {
        let inner = &mut *self.inner.borrow_mut();
        let ring = &mut inner.ring;

        if let Err(e) = ring.submit_and_wait(1) {
            if e.raw_os_error() == Some(libc::EBUSY) {
                return Ok(());
            }
            if e.kind() == std::io::ErrorKind::Interrupted {
                return Ok(());
            }
            return Err(e);
        }

        let mut cq = ring.completion();
        cq.sync();
        for cqe in cq {
            if cqe.user_data() == u64::MAX {
                continue;
            }
            let index = cqe.user_data() as _;
            let action = &mut inner.actions[index];
            if action.complete(cqe) {
                inner.actions.remove(index);
            }
        }
        Ok(())
    }

    pub(crate) fn with<T>(&self, f: impl FnOnce() -> T) -> T {
        CURRENT.set(self, f)
    }

    pub(crate) fn submit<T>(&self, action: T, sqe: Entry) -> std::io::Result<Action<T>> {
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        let key = inner.actions.insert(State::Submitted);

        let ring = &mut inner.ring;
        if ring.submission().is_full() {
            ring.submit()?;
        }
        ring.submission().sync();

        let sqe = sqe.user_data(key as u64);
        unsafe {
            ring.submission().push(&sqe).expect("push entry fail");
        }
        ring.submit()?;
        Ok(Action {
            driver: self.clone(),
            action: Some(action),
            key,
        })
    }
}

enum State {
    /// The operation has been submitted to uring and is currently in-flight
    Submitted,
    /// The submitter is waiting for the completion of the operation
    Waiting(Waker),
    /// The operation has completed.
    Completed(CqeResult),
    /// The operations list.
    CompletionList(Vec<CqeResult>),
    /// Ignored
    Ignored(Box<dyn Any>),
}

impl State {
    fn complete(&mut self, cqe: cqueue::Entry) -> bool {
        match mem::replace(self, State::Submitted) {
            s @ State::Submitted | s @ State::Waiting(..) => {
                if io_uring::cqueue::more(cqe.flags()) {
                    *self = State::CompletionList(vec![cqe.into()]);
                } else {
                    *self = State::Completed(cqe.into());
                }
                if let State::Waiting(waker) = s {
                    waker.wake();
                }
                false
            }
            s @ State::Ignored(..) => {
                if io_uring::cqueue::more(cqe.flags()) {
                    *self = s;
                    false
                } else {
                    true
                }
            }
            State::CompletionList(mut list) => {
                list.push(cqe.into());
                *self = State::CompletionList(list);
                false
            }
            State::Completed(..) => unreachable!("invalid state"),
        }
    }
}
