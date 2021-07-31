use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use io_uring::{opcode, types};

use crate::driver::Action;

pub struct Read {
    buf: Vec<u8>,
}

impl Action<Read> {
    pub fn read(fd: RawFd, len: u32) -> io::Result<Action<Read>> {
        let mut buf = Vec::with_capacity(len as usize);
        let entry = opcode::Read::new(types::Fd(fd), buf.as_mut_ptr(), len).build();
        Action::submit(Read { buf }, entry)
    }

    pub fn poll_read(&mut self, cx: &mut Context) -> Poll<io::Result<Vec<u8>>> {
        let completion = ready!(Pin::new(&mut *self).poll(cx));
        let n = completion.result?;
        let mut action = completion.action;
        unsafe { action.buf.set_len(n as usize) };
        Poll::Ready(Ok(action.buf))
    }
}
