use std::collections::VecDeque;
use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Buf, Completable, CqeResult, Op, BUF_BGID};

pub(crate) struct RecvMulti {
    results: VecDeque<io::Result<Buf>>,
}

impl RecvMulti {
    pub fn next(&mut self) -> Option<io::Result<Buf>> {
        self.results.pop_front()
    }
}

impl Op<RecvMulti> {
    pub(crate) fn recv_multi(fd: RawFd) -> io::Result<Op<RecvMulti>> {
        let entry = opcode::RecvMulti::new(types::Fd(fd), BUF_BGID).build();
        Op::submit(
            RecvMulti {
                results: VecDeque::new(),
            },
            entry,
        )
    }
}

impl Completable for RecvMulti {
    type Output = io::Result<Buf>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        let _ = cqe.result?;
        match cqe.buf {
            Some(buf) => Ok(buf),
            None => Err(io::Error::new(io::ErrorKind::Other, "buf not found")),
        }
    }

    fn update(&mut self, cqe: CqeResult) {
        let buf = cqe.result.and_then(|_| match cqe.buf {
            Some(buf) => Ok(buf),
            None => Err(io::Error::new(io::ErrorKind::Other, "buf not found")),
        });
        self.results.push_back(buf);
    }
}
