use std::io;

use io_uring::{opcode, types};
use socket2::SockAddr;

use crate::driver::{Action, SharedFd};

pub(crate) struct Connect {
    fd: SharedFd,
    sock_addr: SockAddr,
}

impl Action<Connect> {
    pub(crate) fn connect(fd: &SharedFd, sock_addr: SockAddr) -> io::Result<Action<Connect>> {
        let connect = Connect {
            fd: fd.clone(),
            sock_addr,
        };
        let entry = opcode::Connect::new(
            types::Fd(connect.fd.raw_fd()),
            connect.sock_addr.as_ptr(),
            connect.sock_addr.len(),
        )
        .build();
        Action::submit(connect, entry)
    }
}
