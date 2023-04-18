use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use io_uring::{opcode, types};

use crate::driver::Action;

pub(crate) struct Read {
    buf: Vec<u8>,
}

impl Action<Read> {
    pub(crate) fn read(fd: RawFd, len: u32) -> io::Result<Action<Read>> {
        let buf = Vec::with_capacity(len as usize);
        let mut read = Read { buf };
        let entry = opcode::Read::new(types::Fd(fd), read.buf.as_mut_ptr(), len).build();
        Action::submit(read, entry)
    }

    pub(crate) fn poll_read(&mut self, cx: &mut Context) -> Poll<io::Result<Vec<u8>>> {
        let completion = ready!(Pin::new(&mut *self).poll(cx));
        let n = completion.result?;
        let mut action = completion.action;
        unsafe { action.buf.set_len(n as usize) };
        Poll::Ready(Ok(action.buf))
    }
}
