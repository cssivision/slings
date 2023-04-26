use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};
use socket2::SockAddr;

use crate::driver::{Completable, CqeResult, Op};

pub(crate) struct Connect {
    sock_addr: SockAddr,
}

impl Op<Connect> {
    pub(crate) fn connect(fd: RawFd, sock_addr: SockAddr) -> io::Result<Op<Connect>> {
        let connect = Connect { sock_addr };
        let entry = opcode::Connect::new(
            types::Fd(fd),
            connect.sock_addr.as_ptr(),
            connect.sock_addr.len(),
        )
        .build();
        Op::submit(connect, entry)
    }
}

impl Completable for Connect {
    type Output = io::Result<()>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        cqe.result?;
        Ok(())
    }
}
