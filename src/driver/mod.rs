use std::cell::RefCell;
use std::io;
use std::mem;
use std::panic;
use std::rc::Rc;
use std::task::Waker;

use io_uring::{cqueue, opcode::ProvideBuffers, squeue::Entry, IoUring};
use scoped_tls::scoped_thread_local;
use slab::Slab;

pub(crate) mod accept;
pub(crate) mod action;
pub(crate) mod buffers;
pub(crate) mod read;
pub(crate) mod stream;
pub(crate) mod timeout;
pub(crate) mod write;

pub(crate) use action::Action;
use buffers::Buffers;
pub(crate) use read::Read;
pub(crate) use stream::Stream;

scoped_thread_local!(static CURRENT: Driver);

pub(crate) struct Driver {
    pub inner: Rc<RefCell<Inner>>,
}

impl Clone for Driver {
    fn clone(&self) -> Self {
        Driver {
            inner: self.inner.clone(),
        }
    }
}

pub(crate) struct Inner {
    ring: IoUring,
    actions: Slab<State>,
    buffers: Buffers,
}

impl Driver {
    pub(crate) fn new() -> io::Result<Driver> {
        let ring = IoUring::new(256)?;

        // check if IORING_FEAT_FAST_POLL is supported
        if !ring.params().is_feature_fast_poll() {
            panic!("IORING_FEAT_FAST_POLL not supported");
        }

        // check if buffer selection is supported
        let mut probe = io_uring::Probe::new();
        ring.submitter().register_probe(&mut probe).unwrap();
        if !probe.is_supported(ProvideBuffers::CODE) {
            panic!("buffer selection not supported");
        }

        let mut driver = Driver {
            inner: Rc::new(RefCell::new(Inner {
                ring,
                actions: Slab::new(),
                buffers: Buffers::new(256, 4096),
            })),
        };
        driver.provide_buffers()?;

        Ok(driver)
    }

    pub(crate) fn wait(&self) -> io::Result<()> {
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        inner.ring.submit_and_wait(1)?;

        let mut cq = inner.ring.completion();
        cq.sync();

        for cqe in cq {
            let key = cqe.user_data();
            if key == u64::MAX {
                continue;
            }
            let action = &mut inner.actions[key as usize];
            action.complete(cqe);
        }
        Ok(())
    }

    pub(crate) fn with<T>(&self, f: impl FnOnce() -> T) -> T {
        CURRENT.set(&self, f)
    }

    fn provide_buffers(&mut self) -> io::Result<()> {
        let mut inner = self.inner.borrow_mut();
        let buffers = &inner.buffers;
        let entry = ProvideBuffers::new(buffers.mem, buffers.size as i32, buffers.num as u16, 0, 0)
            .build()
            .user_data(0);

        unsafe {
            inner
                .ring
                .submission()
                .push(&entry)
                .expect("push provide_buffers entry fail");
        }

        inner.ring.submit_and_wait(1)?;
        for cqe in inner.ring.completion() {
            let ret = cqe.result();
            if cqe.user_data() != 0 {
                panic!("provide_buffers user_data error");
            }
            if ret < 0 {
                panic!("provide_buffers submit error, ret: {}", ret);
            }
        }
        Ok(())
    }

    pub(crate) fn submit(&self, sqe: Entry) -> io::Result<u64> {
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        let key = inner.actions.insert(State::Submitted) as u64;

        if inner.ring.submission().is_full() {
            inner.ring.submit()?;
            inner.ring.submission().sync();
        }

        let sqe = sqe.user_data(key);
        unsafe {
            inner
                .ring
                .submission()
                .push(&sqe)
                .expect("submit entry fail");
        }
        inner.ring.submit()?;
        Ok(key)
    }
}

#[derive(Debug)]
pub enum State {
    /// The operation has been submitted to uring and is currently in-flight
    Submitted,
    /// The submitter is waiting for the completion of the operation
    Waiting(Waker),
    /// The operation has completed.
    Completed(cqueue::Entry),
}

impl State {
    pub fn complete(&mut self, cqe: cqueue::Entry) {
        match mem::replace(self, State::Submitted) {
            State::Submitted => {
                *self = State::Completed(cqe);
            }
            State::Waiting(waker) => {
                *self = State::Completed(cqe);
                waker.wake();
            }
            State::Completed(_) => unreachable!("invalid operation state"),
        };
    }
}
