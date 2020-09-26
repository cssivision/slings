use std::io;
use std::os::unix::io::RawFd;
use std::sync::{Arc, Mutex};
use std::task::Waker;
use std::thread;

use futures::future::poll_fn;
use io_uring::{
    concurrent, cqueue,
    opcode::{self, types},
    squeue::Entry,
    IoUring,
};
use once_cell::sync::Lazy;
use slab::Slab;

#[derive(Debug)]
enum Action {
    Accept {
        inner: Mutex<AcceptAction>,
    },
    Read {
        fd: RawFd,
        buf_index: usize,
        waker: Option<Waker>,
    },
    Write {
        fd: RawFd,
        buf_index: usize,
        offset: usize,
        len: usize,
        waker: Option<Waker>,
    },
}

impl Action {
    fn handle(&self, wakers: &mut Vec<Waker>, cqe: cqueue::Entry) {
        match self {
            Action::Accept { inner } => {
                let mut action = inner.lock().unwrap();
                if let Some(w) = action.waker.take() {
                    wakers.push(w);
                }

                action.fd = Some(cqe.result());
            }
            _ => {}
        }
    }
}

#[derive(Debug)]
struct AcceptAction {
    waker: Option<Waker>,
    fd: Option<i32>,
}

struct Completion {
    ring: concurrent::IoUring,
    actions: Mutex<Slab<Arc<Action>>>,
    bufpool: Mutex<Slab<Box<[u8]>>>,
    free_bufs: Vec<usize>,
}

impl Completion {
    fn get() -> &'static Completion {
        static COMPLETION: Lazy<Completion> = Lazy::new(|| {
            thread::spawn(move || {
                let completion = Completion::get();

                loop {}
            });

            Completion {
                ring: IoUring::new(256).expect("init io uring fail").concurrent(),
                actions: Mutex::new(Slab::new()),
                bufpool: Mutex::new(Slab::new()),
                free_bufs: Vec::new(),
            }
        });

        &COMPLETION
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
                action.handle(&mut wakers, cqe);
            }
        }

        Ok(())
    }

    fn submit(&self, sqe: Entry) -> io::Result<()> {
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
}

fn other(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, msg)
}
