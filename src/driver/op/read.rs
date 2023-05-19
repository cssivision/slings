use std::io;
use std::os::unix::io::RawFd;
use std::ptr;

use io_uring::{opcode, squeue, types};

use crate::driver::{Buf, Completable, CqeResult, Op, BUF_BGID};

pub(crate) struct Read;

impl Op<Read> {
    pub(crate) fn read(fd: RawFd, len: u32) -> io::Result<Op<Read>> {
        let entry = opcode::Read::new(types::Fd(fd), ptr::null_mut(), len)
            .buf_group(BUF_BGID)
            .build()
            .flags(squeue::Flags::BUFFER_SELECT);
        Op::submit(Read, entry)
    }
}

impl Completable for Read {
    type Output = io::Result<Buf>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        let _ = cqe.result?;
        match cqe.buf {
            Some(buf) => Ok(buf),
            None => Err(io::Error::new(io::ErrorKind::Other, "buf not found")),
        }
    }
}
