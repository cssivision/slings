use std::cell::RefCell;
use std::io;
use std::mem;
use std::panic;
use std::rc::Rc;
use std::task::Waker;

use io_uring::{cqueue, opcode::ProvideBuffers, squeue::Entry, IoUring};
use scoped_tls::scoped_thread_local;
use slab::Slab;

pub(crate) mod action;
pub(crate) use action::Action;
pub(crate) mod accept;

scoped_thread_local!(static CURRENT: Driver);

pub(crate) struct Driver {
    inner: Rc<RefCell<Inner>>,
}

struct Inner {
    ring: IoUring,
    actions: Slab<State>,
    buffers: Vec<u8>,
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

        let driver = Driver {
            inner: Rc::new(RefCell::new(Inner {
                ring,
                actions: Slab::new(),
                buffers: Vec::with_capacity(4096 * 256),
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
            let key = cqe.user_data() as usize;
            let action = &mut inner.actions[key];
            action.complete(cqe);
        }

        Ok(())
    }

    pub(crate) fn with<T>(&self, f: impl FnOnce() -> T) -> T {
        CURRENT.set(&self, f)
    }

    fn provide_buffers(&self) -> io::Result<()> {
        let mut inner = self.inner.borrow_mut();
        let buffers = inner.buffers.as_mut_ptr() as *mut u8;
        let entry = ProvideBuffers::new(buffers, 4096 * 256, 256, 0, 0)
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
            if ret < 0 {
                panic!("provide_buffers submit error, ret: {}", ret);
            }
        }
        Ok(())
    }

    pub(crate) fn submit(&self, sqe: Entry) -> io::Result<u64> {
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        let entry = inner.actions.vacant_entry();
        let key = entry.key() as u64;

        if inner.ring.submission().is_full() {
            inner.ring.submit()?;
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
        let state = State::Submitted;
        entry.insert(state);
        Ok(key)
    }
}

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
            State::Submitted => State::Completed(cqe),
            State::Waiting(waker) => {
                waker.wake();
                State::Completed(cqe)
            }
            State::Completed(_) => unreachable!("invalid operation state"),
        };
    }
}
