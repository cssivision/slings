use std::collections::VecDeque;
use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Completable, CqeResult, Op};

pub(crate) struct AcceptMulti {
    results: VecDeque<io::Result<RawFd>>,
}

impl AcceptMulti {
    pub fn next(&mut self) -> Option<io::Result<RawFd>> {
        self.results.pop_front()
    }
}

impl Op<AcceptMulti> {
    pub(crate) fn accept_multi(fd: RawFd) -> io::Result<Op<AcceptMulti>> {
        let entry = opcode::AcceptMulti::new(types::Fd(fd))
            .flags(libc::SOCK_CLOEXEC)
            .build();
        Op::submit(
            AcceptMulti {
                results: VecDeque::new(),
            },
            entry,
        )
    }
}

impl Completable for AcceptMulti {
    type Output = io::Result<RawFd>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        Ok(cqe.result? as i32)
    }

    fn update(&mut self, cqe: CqeResult) {
        let fd = cqe.result.map(|v| v as i32);
        self.results.push_back(fd);
    }
}
