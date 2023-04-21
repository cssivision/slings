use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Action, Completable, CqeResult};

#[allow(dead_code)]
pub(crate) struct Send {
    buf: Vec<u8>,
}

impl Action<Send> {
    pub(crate) fn send(fd: RawFd, buf: &[u8]) -> io::Result<Action<Send>> {
        let buf = buf.to_vec();
        let entry = opcode::Send::new(types::Fd(fd), buf.as_ptr(), buf.len() as u32).build();
        Action::submit(Send { buf }, entry)
    }
}

impl Completable for Send {
    type Output = io::Result<usize>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        let n = cqe.result? as usize;
        Ok(n)
    }
}
