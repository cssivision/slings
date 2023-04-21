use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Action, Completable, CqeResult};

pub(crate) struct Write {
    buf: Vec<u8>,
}

impl Action<Write> {
    pub(crate) fn write(fd: RawFd, buf: &[u8]) -> io::Result<Action<Write>> {
        let buf = buf.to_vec();
        let write = Write { buf };
        let entry =
            opcode::Write::new(types::Fd(fd), write.buf.as_ptr(), write.buf.len() as u32).build();
        Action::submit(write, entry)
    }
}

impl Completable for Write {
    type Output = io::Result<usize>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        let n = cqe.result? as usize;
        Ok(n)
    }
}
