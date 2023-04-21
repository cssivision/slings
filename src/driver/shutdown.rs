use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Action, Completable, CqeResult};

pub(crate) struct Shutdown;

impl Action<Shutdown> {
    pub(crate) fn shutdown(fd: RawFd, how: libc::c_int) -> io::Result<Action<Shutdown>> {
        let shutdown = Shutdown;
        let entry = opcode::Shutdown::new(types::Fd(fd), how).build();
        Action::submit(shutdown, entry)
    }
}

impl Completable for Shutdown {
    type Output = io::Result<usize>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        let n = cqe.result? as usize;
        Ok(n)
    }
}
