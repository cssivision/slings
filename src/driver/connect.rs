use std::io;
use std::net::SocketAddr;

use io_uring::{opcode, types};
use os_socketaddr::OsSocketAddr;

use crate::driver::{Action, SharedFd};

pub struct Connect {
    fd: SharedFd,
    os_socket_addr: OsSocketAddr,
}

impl Action<Connect> {
    pub fn connect(fd: &SharedFd, socket_addr: SocketAddr) -> io::Result<Action<Connect>> {
        let os_socket_addr = OsSocketAddr::from(socket_addr);
        let connect = Connect {
            fd: fd.clone(),
            os_socket_addr,
        };
        let entry = opcode::Connect::new(
            types::Fd(connect.fd.raw_fd()),
            connect.os_socket_addr.as_ptr(),
            connect.os_socket_addr.len(),
        )
        .build();
        Action::submit(connect, entry)
    }
}
