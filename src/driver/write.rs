use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use io_uring::{opcode, types};

use crate::driver::{Action, SharedFd};

#[allow(dead_code)]
pub struct Write {
    fd: SharedFd,
    buf: Vec<u8>,
}

impl Action<Write> {
    pub fn write(fd: &SharedFd, buf: &[u8]) -> io::Result<Action<Write>> {
        let buf = buf.to_vec();
        let write = Write {
            buf,
            fd: fd.clone(),
        };
        let entry = opcode::Write::new(
            types::Fd(write.fd.raw_fd()),
            write.buf.as_ptr(),
            write.buf.len() as u32,
        )
        .build();
        Action::submit(write, entry)
    }

    pub(crate) fn poll_write(&mut self, cx: &mut Context) -> Poll<io::Result<usize>> {
        let complete = ready!(Pin::new(self).poll(cx));
        let n = complete.result? as usize;
        Poll::Ready(Ok(n))
    }
}
