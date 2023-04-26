use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Completable, CqeResult, Op};

pub(crate) struct Write {
    buf: Vec<u8>,
}

impl Op<Write> {
    pub(crate) fn write(fd: RawFd, buf: &[u8]) -> io::Result<Op<Write>> {
        let buf = buf.to_vec();
        let write = Write { buf };
        let entry =
            opcode::Write::new(types::Fd(fd), write.buf.as_ptr(), write.buf.len() as u32).build();
        Op::submit(write, entry)
    }
}

impl Completable for Write {
    type Output = io::Result<usize>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        let n = cqe.result? as usize;
        Ok(n)
    }
}
