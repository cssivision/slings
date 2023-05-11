use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Completable, CqeResult, Op};

pub(crate) struct RecvMulti;

impl Op<RecvMulti> {
    pub(crate) fn recv_multi(fd: RawFd, buf_group: u16) -> io::Result<Op<RecvMulti>> {
        let entry = opcode::RecvMulti::new(types::Fd(fd), buf_group).build();
        Op::submit(RecvMulti, entry)
    }
}

impl Completable for RecvMulti {
    type Output = io::Result<()>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        let n = cqe.result? as usize;
        Ok(())
    }
}
