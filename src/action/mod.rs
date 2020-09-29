use std::os::unix::io::RawFd;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Waker;

use io_uring::cqueue;

mod accept;

use accept::AcceptAction;

pub enum Action {
    Accept {
        inner: Arc<Mutex<AcceptAction>>,
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
                let ret = cqe.result();
                let mut action = inner.lock().unwrap();
                action.fd = Some(ret);
                if let Some(w) = action.waker.take() {
                    wakers.push(w);
                }

                action.fd = Some(cqe.result());
            }
            _ => {}
        }
    }
}
