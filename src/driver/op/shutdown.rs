use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Completable, CqeResult, Op};

pub(crate) struct Shutdown;

impl Op<Shutdown> {
    pub(crate) fn shutdown(fd: RawFd, how: libc::c_int) -> io::Result<Op<Shutdown>> {
        let shutdown = Shutdown;
        let entry = opcode::Shutdown::new(types::Fd(fd), how).build();
        Op::submit(shutdown, entry)
    }
}

impl Completable for Shutdown {
    type Output = io::Result<usize>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        let n = cqe.result? as usize;
        Ok(n)
    }
}
