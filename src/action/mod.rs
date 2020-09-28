use std::os::unix::io::RawFd;
use std::sync::Mutex;
use std::task::Waker;

use io_uring::cqueue;

mod accept;

use accept::AcceptAction;

#[derive(Debug)]
pub enum Action {
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
    ProvideBuf,
}

impl Action {
    pub fn trigger(&self, wakers: &mut Vec<Waker>, cqe: cqueue::Entry) {
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
