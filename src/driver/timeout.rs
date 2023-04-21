use std::io;

use io_uring::{opcode, types};

use crate::driver::{Action, Completable, CqeResult};

pub(crate) struct Timeout {
    spec: types::Timespec,
}

impl Action<Timeout> {
    pub(crate) fn timeout(sec: u64, nsec: u32) -> io::Result<Action<Timeout>> {
        let timeout = Timeout {
            spec: types::Timespec::new().sec(sec).nsec(nsec),
        };
        let entry = opcode::Timeout::new(&timeout.spec as *const _).build();
        Action::submit(timeout, entry)
    }
}

impl Completable for Timeout {
    type Output = io::Result<()>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        match cqe.result {
            Err(err) if err.raw_os_error() == Some(libc::ETIME) => Ok(()),
            Err(err) => Err(err),
            Ok(n) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("result {}", n),
            )),
        }
    }
}
