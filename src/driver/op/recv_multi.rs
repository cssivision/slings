use std::collections::VecDeque;
use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Completable, CqeResult, Op, BUF_BGID};

pub(crate) struct RecvMulti {
    results: VecDeque<CqeResult>,
}

impl RecvMulti {
    pub fn next(&mut self) -> Option<CqeResult> {
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
    type Output = CqeResult;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        cqe
    }

    fn update(&mut self, cqe: CqeResult) {
        self.results.push_back(cqe);
    }
}
