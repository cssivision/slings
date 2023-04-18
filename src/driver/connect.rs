use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};
use socket2::SockAddr;

use crate::driver::Action;

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
