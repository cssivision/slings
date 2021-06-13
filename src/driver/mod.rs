use std::io;
use std::panic;

use io_uring::{opcode::ProvideBuffers, squeue::Entry, IoUring};
use scoped_tls::scoped_thread_local;
use slab::Slab;

mod action;
use action::Action;

scoped_thread_local!(static CURRENT: Driver);

pub struct Driver {
    ring: IoUring,
    actions: Slab<Action>,
    buffers: Vec<u8>,
}

impl Driver {
    pub fn new() -> io::Result<Driver> {
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
            ring,
            actions: Slab::new(),
            buffers: Vec::with_capacity(4096 * 256),
        };
        driver.provide_buffers()?;

        Ok(driver)
    }

    fn wait(&self) -> io::Result<()> {
        self.ring.submit_and_wait(1)?;
        for cqe in self.ring.completion() {
            let key = cqe.user_data() as usize;
            if let Some(action) = self.actions.get(key) {}
        }

        Ok(())
    }

    pub fn with<T>(&self, f: impl FnOnce() -> T) -> T {
        CURRENT.set(&self, f)
    }

    fn provide_buffers(&mut self) -> io::Result<()> {
        let buffers = self.buffers.as_mut_ptr() as *mut u8;
        let entry = ProvideBuffers::new(buffers, 4096 * 256, 256, 0, 0)
            .build()
            .user_data(0);

        unsafe {
            self.ring
                .submission()
                .push(&entry)
                .expect("push provide_buffers entry fail");
        }

        self.ring.submit_and_wait(1)?;
        for cqe in self.ring.completion() {
            let ret = cqe.result();
            if ret < 0 {
                panic!("provide_buffers submit error, ret: {}", ret);
            }
        }
        Ok(())
    }

    pub fn submit(&self, sqe: Entry) -> io::Result<()> {
        if self.ring.submission().is_full() {
            self.ring.submit()?;
        }
        unsafe {
            self.ring
                .submission()
                .push(&sqe)
                .map_err(|_| other("sq push fail"))?;
        }
        self.ring.submit()?;
        Ok(())
    }
}
