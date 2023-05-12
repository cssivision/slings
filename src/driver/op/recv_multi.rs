use std::collections::VecDeque;
use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Completable, CqeResult, Op};

pub(crate) struct RecvMulti {
    results: VecDeque<CqeResult>,
}

impl Op<RecvMulti> {
    pub(crate) fn recv_multi(fd: RawFd, buf_group: u16) -> io::Result<Op<RecvMulti>> {
        let entry = opcode::RecvMulti::new(types::Fd(fd), buf_group).build();
        Op::submit(
            RecvMulti {
                results: VecDeque::new(),
            },
            entry,
        )
    }
}

impl Completable for RecvMulti {
    type Output = CqeResult;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        cqe
    }

    fn update(&mut self, cqe: CqeResult) {
        self.results.push_back(cqe);
    }
}
