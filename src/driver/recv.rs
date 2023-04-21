use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Action, Completable, CqeResult};

pub(crate) struct Recv {
    buf: Vec<u8>,
}

impl Action<Recv> {
    pub(crate) fn recv(fd: RawFd, len: usize) -> io::Result<Action<Recv>> {
        let mut buf = Vec::with_capacity(len);
        let entry = opcode::Recv::new(types::Fd(fd), buf.as_mut_ptr(), len as u32).build();
        Action::submit(Recv { buf }, entry)
    }
}

impl Completable for Recv {
    type Output = io::Result<Vec<u8>>;

    fn complete(mut self, cqe: CqeResult) -> Self::Output {
        let n = cqe.result?;
        unsafe { self.buf.set_len(n as usize) };
        Ok(self.buf)
    }
}
