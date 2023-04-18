use std::any::Any;
use std::cell::RefCell;
use std::io;
use std::mem;
use std::panic;
use std::rc::Rc;
use std::task::Waker;

use io_uring::squeue::Entry;
use io_uring::{cqueue, IoUring};
use scoped_tls::scoped_thread_local;
use slab::Slab;

mod accept;
mod action;
mod close;
mod connect;
mod read;
mod recv;
mod recvmsg;
mod send;
mod sendmsg;
mod shared_fd;
mod shutdown;
mod timeout;
mod write;

pub(crate) use action::Action;
pub(crate) use read::Read;
pub(crate) use recv::Recv;
pub(crate) use recvmsg::RecvMsg;
pub(crate) use send::Send;
pub(crate) use sendmsg::SendMsg;
pub(crate) use shared_fd::SharedFd;
pub(crate) use shutdown::Shutdown;
pub(crate) use timeout::Timeout;
pub(crate) use write::Write;

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
    pub(crate) fn new() -> io::Result<Driver> {
        let ring = IoUring::new(256)?;
        // check if IORING_FEAT_FAST_POLL is supported
        if !ring.params().is_feature_fast_poll() {
            panic!("IORING_FEAT_FAST_POLL not supported");
        }

        let driver = Driver {
            inner: Rc::new(RefCell::new(Inner {
                ring,
                actions: Slab::with_capacity(256),
            })),
        };
        Ok(driver)
    }

    pub(crate) fn wait(&self) -> io::Result<()> {
        let inner = &mut *self.inner.borrow_mut();
        let ring = &mut inner.ring;

        if let Err(e) = ring.submit_and_wait(1) {
            if e.raw_os_error() == Some(libc::EBUSY) {
                return Ok(());
            }
            if e.kind() == io::ErrorKind::Interrupted {
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

    pub(crate) fn try_submit<T>(&self, action: T, sqe: Entry) -> io::Result<Action<T>> {
        if self.inner.try_borrow_mut().is_err() {
            return Err(io::ErrorKind::Other.into());
        }
        self.submit(action, sqe)
    }

    pub(crate) fn submit<T>(&self, action: T, sqe: Entry) -> io::Result<Action<T>> {
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
    Completed(cqueue::Entry),
    /// Ignored
    Ignored(Box<dyn Any>),
}

impl State {
    fn complete(&mut self, cqe: cqueue::Entry) -> bool {
        match mem::replace(self, State::Submitted) {
            State::Submitted => {
                *self = State::Completed(cqe);
                false
            }
            State::Waiting(waker) => {
                *self = State::Completed(cqe);
                waker.wake();
                false
            }
            State::Ignored(..) => true,
            State::Completed(..) => unreachable!("invalid operation state"),
        }
    }
}
