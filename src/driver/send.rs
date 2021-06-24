use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use io_uring::{opcode, types};

use crate::driver::Action;

pub struct Send {
    _buf: Vec<u8>,
}

impl Action<Send> {
    pub fn send(fd: RawFd, buf: &[u8]) -> io::Result<Action<Send>> {
        let buf = buf.to_vec();
        let ptr = buf.as_ptr();
        let len = buf.len() as u32;
        let entry = opcode::Send::new(types::Fd(fd), ptr, len).build();
        Action::submit(Send { _buf: buf }, entry)
    }

    pub(crate) fn poll_send(&mut self, cx: &mut Context) -> Poll<io::Result<usize>> {
        let complete = ready!(Pin::new(self).poll(cx));
        let n = complete.result? as usize;
        Poll::Ready(Ok(n))
    }
}
