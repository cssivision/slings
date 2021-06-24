use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use io_uring::{opcode, types};

use crate::driver::Action;

pub struct Write {
    _buf: Vec<u8>,
}

impl Action<Write> {
    pub fn write(fd: RawFd, buf: &[u8]) -> io::Result<Action<Write>> {
        let buf = buf.to_vec();
        let ptr = buf.as_ptr();
        let len = buf.len() as u32;
        let entry = opcode::Write::new(types::Fd(fd), ptr, len).build();
        Action::submit(Write { _buf: buf }, entry)
    }

    pub(crate) fn poll_write(&mut self, cx: &mut Context) -> Poll<io::Result<usize>> {
        let complete = ready!(Pin::new(self).poll(cx));
        let n = complete.result? as usize;
        Poll::Ready(Ok(n))
    }
}
