use std::any::Any;
use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::mem;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use io_uring::squeue::Entry;
use io_uring::{cqueue, IoUring};
use scoped_tls::scoped_thread_local;
use slab::Slab;

mod op;

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

    pub(crate) fn submit<T>(&self, action: T, sqe: Entry) -> std::io::Result<Op<T>> {
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
        Ok(Op {
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

pub(crate) trait Completable {
    type Output;
    /// `complete` will be called for cqe's do not have the `more` flag set
    fn complete(self, cqe: CqeResult) -> Self::Output;
    /// Update will be called for cqe's which have the `more` flag set.
    /// The Op should update any internal state as required.
    fn update(&mut self, _cqe: CqeResult) {}
}

pub(crate) struct Op<T: 'static> {
    pub driver: Driver,
    pub action: Option<T>,
    pub key: usize,
}

impl<T> Op<T> {
    pub(crate) fn get_mut(&mut self) -> &mut T {
        self.action.as_mut().unwrap()
    }

    pub(crate) fn submit(action: T, entry: Entry) -> io::Result<Op<T>> {
        CURRENT.with(|driver| driver.submit(action, entry))
    }

    pub(crate) fn insert_waker(&self, waker: Waker) {
        let mut inner = self.driver.inner.borrow_mut();
        let state = inner.actions.get_mut(self.key).expect("invalid state key");
        *state = State::Waiting(waker);
    }

    fn poll2(&mut self, cx: &mut Context) -> Poll<T::Output>
    where
        T: Completable,
    {
        let mut inner = self.driver.inner.borrow_mut();
        let state = inner.actions.get_mut(self.key).expect("invalid state key");

        match mem::replace(state, State::Submitted) {
            State::Submitted => {
                *state = State::Waiting(cx.waker().clone());
                Poll::Pending
            }
            State::Waiting(waker) => {
                if !waker.will_wake(cx.waker()) {
                    *state = State::Waiting(cx.waker().clone());
                } else {
                    *state = State::Waiting(waker);
                }
                Poll::Pending
            }
            State::Completed(cqe) => {
                inner.actions.remove(self.key);
                Poll::Ready(self.action.take().unwrap().complete(cqe))
            }
            State::CompletionList(list) => {
                let data = self.action.as_mut().unwrap();
                let mut status = None;
                let mut updated = false;
                for cqe in list.into_iter() {
                    if cqueue::more(cqe.flags) {
                        updated = true;
                        data.update(cqe);
                    } else {
                        status = Some(cqe);
                        break;
                    }
                }
                if updated {
                    // because we update internal state, wake and rerun the task.
                    cx.waker().wake_by_ref();
                }
                match status {
                    None => {
                        *state = State::Waiting(cx.waker().clone());
                    }
                    Some(cqe) => {
                        *state = State::Completed(cqe);
                    }
                }
                Poll::Pending
            }
            State::Ignored(..) => unreachable!(),
        }
    }
}

impl<T> Drop for Op<T> {
    fn drop(&mut self) {
        let mut inner = self.driver.inner.borrow_mut();
        let state = match inner.actions.get_mut(self.key) {
            Some(s) => s,
            None => return,
        };

        match state {
            State::Submitted | State::Waiting(_) => {
                *state = State::Ignored(Box::new(self.action.take()));
            }
            State::Completed(..) => {
                inner.actions.remove(self.key);
            }
            State::CompletionList(list) => {
                let more = if !list.is_empty() {
                    io_uring::cqueue::more(list.last().unwrap().flags)
                } else {
                    false
                };
                if more {
                    *state = State::Ignored(Box::new(self.action.take()));
                } else {
                    inner.actions.remove(self.key);
                }
            }
            State::Ignored(..) => unreachable!(),
        }
    }
}

impl<T> Future for Op<T>
where
    T: Unpin + Completable,
{
    type Output = T::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.poll2(cx)
    }
}

#[allow(dead_code)]
pub(crate) struct CqeResult {
    pub(crate) result: io::Result<u32>,
    pub(crate) flags: u32,
}

impl From<cqueue::Entry> for CqeResult {
    fn from(cqe: cqueue::Entry) -> Self {
        let res = cqe.result();
        let flags = cqe.flags();
        let result = if res >= 0 {
            Ok(res as u32)
        } else {
            Err(io::Error::from_raw_os_error(-res))
        };
        CqeResult { result, flags }
    }
}
