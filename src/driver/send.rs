use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use io_uring::{opcode, types};

use crate::driver::Action;

#[allow(dead_code)]
pub(crate) struct Send {
    buf: Vec<u8>,
}

impl Action<Send> {
    pub(crate) fn send(fd: RawFd, buf: &[u8]) -> io::Result<Action<Send>> {
        let buf = buf.to_vec();
        let entry = opcode::Send::new(types::Fd(fd), buf.as_ptr(), buf.len() as u32).build();
        Action::submit(Send { buf }, entry)
    }

    pub(crate) fn poll_send(&mut self, cx: &mut Context) -> Poll<io::Result<usize>> {
        let complete = ready!(Pin::new(self).poll(cx));
        let n = complete.result? as usize;
        Poll::Ready(Ok(n))
    }
}
