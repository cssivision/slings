use std::io;
use std::mem::transmute;
use std::os::unix::io::RawFd;
use std::panic;
use std::ptr;
use std::sync::{Arc, Mutex};
use std::task::Waker;
use std::thread;

use crate::action::Action;
use crate::other;

use io_uring::{
    concurrent,
    opcode::{self, types},
    squeue::Entry,
    IoUring,
};
use once_cell::sync::Lazy;
use slab::Slab;

const MAX_MSG_LEN: i32 = 2048;
const BUFFERS_COUNT: u16 = 4096;
const GROUP_ID: u16 = 1028;

pub struct Completion {
    ring: concurrent::IoUring,
    actions: Mutex<Slab<Arc<Action>>>,
    buffers: Vec<Vec<u8>>,
}

impl Completion {
    pub fn get() -> &'static Completion {
        static COMPLETION: Lazy<Completion> = Lazy::new(|| {
            thread::spawn(move || {
                let completion = Completion::get();

                loop {
                    match completion.wait() {
                        Ok(_) => {}
                        Err(e) => {
                            log::trace!("completion wait err: {:?}", e);
                        }
                    }
                }
            });

            let ring = IoUring::new(256).expect("init io uring fail");

            // check if IORING_FEAT_FAST_POLL is supported
            if !ring.params().is_feature_fast_poll() {
                panic!("IORING_FEAT_FAST_POLL not supported");
            }

            // check if buffer selection is supported
            let mut probe = io_uring::Probe::new();
            ring.submitter().register_probe(&mut probe).unwrap();
            if !probe.is_supported(opcode::ProvideBuffers::CODE) {
                panic!("buffer selection not supported");
            }

            let ring = ring.concurrent();
            let mut c = Completion {
                ring,
                actions: Mutex::new(Slab::new()),
                buffers: vec![vec![0u8; MAX_MSG_LEN as usize]; BUFFERS_COUNT as usize],
            };

            c.setup().unwrap();
            c
        });

        &COMPLETION
    }

    fn setup(&mut self) -> io::Result<()> {
        let buffers: *mut u8 =
            unsafe { transmute::<*mut Vec<u8>, *mut u8>(self.buffers.as_mut_ptr()) };

        let entry =
            opcode::ProvideBuffers::new(buffers, MAX_MSG_LEN, BUFFERS_COUNT, GROUP_ID, 0).build();

        let sq = self.ring.submission();
        unsafe {
            sq.push(entry)
                .map_err(|_| other("push provide_buffers entry fail"))?;
        }

        self.ring.submit_and_wait(1)?;
        if let Some(cqe) = self.ring.completion().pop() {
            let ret = cqe.result();
            if ret < 0 {
                return Err(other(&format!(
                    "provide_buffers submit error, ret: {}",
                    ret
                )));
            }
        }

        Ok(())
    }

    fn wait(&self) -> io::Result<()> {
        self.ring.submit_and_wait(1)?;
        let mut wakers = Vec::new();
        let mut actions = self.actions.lock().unwrap();

        while let Some(cqe) = self.ring.completion().pop() {
            let ret = cqe.result();
            if ret < 0 {
                continue;
            }

            let key = cqe.user_data() as usize;

            if actions.contains(key) {
                let action = actions.remove(key);
                action.trigger(&mut wakers, cqe);
            }
        }

        for waker in wakers {
            let _ = panic::catch_unwind(|| waker.wake());
        }

        Ok(())
    }

    pub fn submit(&self, sqe: Entry) -> io::Result<()> {
        let sq = self.ring.submission();

        if sq.is_full() {
            self.ring.submit()?;
        }

        unsafe {
            sq.push(sqe).map_err(|_| other("sq push fail"))?;
        }

        self.ring.submit()?;
        Ok(())
    }

    fn insert(&self, action: Action) -> usize {
        let mut actions = self.actions.lock().unwrap();
        actions.insert(Arc::new(action))
    }
}
