use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Completable, CqeResult, Op};

pub(crate) struct Recv {
    buf: Vec<u8>,
}

impl Op<Recv> {
    pub(crate) fn recv(fd: RawFd, len: usize) -> io::Result<Op<Recv>> {
        let mut buf = Vec::with_capacity(len);
        let entry = opcode::Recv::new(types::Fd(fd), buf.as_mut_ptr(), len as u32).build();
        Op::submit(Recv { buf }, entry)
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
