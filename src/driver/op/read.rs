use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Completable, CqeResult, Op};

pub(crate) struct Read {
    buf: Vec<u8>,
}

impl Op<Read> {
    pub(crate) fn read(fd: RawFd, len: u32) -> io::Result<Op<Read>> {
        let buf = Vec::with_capacity(len as usize);
        let mut read = Read { buf };
        let entry = opcode::Read::new(types::Fd(fd), read.buf.as_mut_ptr(), len).build();
        Op::submit(read, entry)
    }
}

impl Completable for Read {
    type Output = io::Result<Vec<u8>>;

    fn complete(mut self, cqe: CqeResult) -> Self::Output {
        let n = cqe.result?;
        unsafe { self.buf.set_len(n as usize) };
        Ok(self.buf)
    }
}
