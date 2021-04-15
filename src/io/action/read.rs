use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::ptr;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use super::Action;
use crate::io::completion::{Completion, GROUP_ID, MAX_MSG_LEN};
use crate::other;

use io_uring::{opcode, squeue::Flags, types};

pub struct ReadAction {
    pub ret: Option<io::Result<i32>>,
    pub waker: Option<Waker>,
    pub flags: u32,
}

pub struct Read {
    key: usize,
    action: Arc<Mutex<ReadAction>>,
}

impl Drop for Read {
    fn drop(&mut self) {
        Completion::get().remove(self.key);
    }
}

impl Read {
    fn poll_read(&self, cx: &mut Context) -> Poll<io::Result<(Vec<u8>, usize)>> {
        let mut action = self.action.lock().unwrap();
        if let Some(ret) = action.ret.take() {
            match ret {
                Ok(ret) => {
                    let n = ret as usize;
                    if n == 0 {
                        return Poll::Ready(Ok((vec![], n)));
                    }

                    let buf_index = action.flags >> 16;
                    let data = Completion::get()
                        .get_data(buf_index as usize, n)
                        .unwrap_or_else(|| vec![]);

                    if data.len() != n {
                        return Poll::Ready(Err(other("data length invalid")));
                    }

                    return Poll::Ready(Ok((data, n)));
                }
                Err(e) => return Poll::Ready(Err(e)),
            }
        }

        if action.waker.is_none() {
            action.waker = Some(cx.waker().clone());
        }

        Poll::Pending
    }
}

pub fn read(fd: RawFd) -> io::Result<Read> {
    let action = Arc::new(Mutex::new(ReadAction {
        waker: None,
        ret: None,
        flags: 0,
    }));

    let key = Completion::get().insert(Arc::new(Action::Read {
        inner: action.clone(),
    }));

    let entry = opcode::Read::new(types::Fd(fd), ptr::null_mut(), MAX_MSG_LEN as u32)
        .buf_group(GROUP_ID)
        .build()
        .flags(Flags::BUFFER_SELECT)
        .user_data(key as _);

    Completion::get().submit(entry)?;

    Ok(Read { key, action })
}

impl Future for Read {
    type Output = io::Result<(Vec<u8>, usize)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_read(cx)
    }
}
