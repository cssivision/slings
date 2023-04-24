use std::collections::VecDeque;
use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Action, Completable, CqeResult};

pub(crate) struct AcceptMulti {
    results: VecDeque<CqeResult>,
}

impl AcceptMulti {
    pub fn next(&mut self) -> Option<CqeResult> {
        self.results.pop_front()
    }
}

impl Action<AcceptMulti> {
    pub(crate) fn accept_multi(fd: RawFd) -> io::Result<Action<AcceptMulti>> {
        let entry = opcode::AcceptMulti::new(types::Fd(fd))
            .flags(libc::SOCK_CLOEXEC)
            .build();
        Action::submit(
            AcceptMulti {
                results: VecDeque::new(),
            },
            entry,
        )
    }
}

impl Completable for AcceptMulti {
    type Output = CqeResult;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        cqe
    }

    fn update(&mut self, cqe: CqeResult) {
        self.results.push_back(cqe);
    }
}
