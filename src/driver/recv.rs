use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use io_uring::{opcode, types};

use crate::driver::{Action, SharedFd};

#[allow(dead_code)]
pub struct Recv {
    fd: SharedFd,
    buf: Vec<u8>,
}

impl Action<Recv> {
    pub fn recv(fd: &SharedFd, len: usize) -> io::Result<Action<Recv>> {
        let mut buf = Vec::with_capacity(len);
        let entry = opcode::Recv::new(types::Fd(fd.raw_fd()), buf.as_mut_ptr(), len as u32).build();
        Action::submit(
            Recv {
                buf,
                fd: fd.clone(),
            },
            entry,
        )
    }

    pub fn poll_recv(&mut self, cx: &mut Context, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let completion = ready!(Pin::new(&mut *self).poll(cx));
        let n = completion.result? as usize;
        let mut action = completion.action;
        unsafe { action.buf.set_len(n as usize) };
        buf[..n].copy_from_slice(&action.buf[..n]);
        Poll::Ready(Ok(n))
    }
}
