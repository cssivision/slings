use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};
use socket2::SockAddr;

use crate::driver::{Action, Completable, CqeResult};

pub(crate) struct Connect {
    sock_addr: SockAddr,
}

impl Action<Connect> {
    pub(crate) fn connect(fd: RawFd, sock_addr: SockAddr) -> io::Result<Action<Connect>> {
        let connect = Connect { sock_addr };
        let entry = opcode::Connect::new(
            types::Fd(fd),
            connect.sock_addr.as_ptr(),
            connect.sock_addr.len(),
        )
        .build();
        Action::submit(connect, entry)
    }
}

impl Completable for Connect {
    type Output = io::Result<()>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        cqe.result?;
        Ok(())
    }
}
