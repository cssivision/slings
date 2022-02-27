use std::io;
use std::net::SocketAddr;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};
use os_socketaddr::OsSocketAddr;

use crate::driver::Action;

pub struct Connect {
    fd: RawFd,
    os_socket_addr: OsSocketAddr,
}

impl Action<Connect> {
    pub fn connect(socket_addr: SocketAddr) -> io::Result<Action<Connect>> {
        let fd = match socket_addr {
            SocketAddr::V4(_) => new_v4_socket(),
            SocketAddr::V6(_) => new_v6_socket(),
        }?;
        let os_socket_addr = OsSocketAddr::from(socket_addr);
        let connect = Connect { fd, os_socket_addr };
        let entry = opcode::Connect::new(
            types::Fd(fd),
            connect.os_socket_addr.as_ptr(),
            connect.os_socket_addr.len(),
        )
        .build();
        Action::submit(connect, entry)
    }
}

impl Connect {
    pub fn get_socket(&self, result: io::Result<i32>) -> io::Result<RawFd> {
        match result {
            Err(err) if err.raw_os_error() != Some(libc::EINPROGRESS) => Err(err),
            _ => Ok(self.fd),
        }
    }
}

pub fn new_v4_socket() -> io::Result<i32> {
    new_socket(libc::AF_INET, libc::SOCK_STREAM)
}

pub fn new_v6_socket() -> io::Result<i32> {
    new_socket(libc::AF_INET6, libc::SOCK_STREAM)
}

pub fn new_socket(domain: libc::c_int, socket_type: libc::c_int) -> io::Result<libc::c_int> {
    let socket_type = socket_type | libc::SOCK_CLOEXEC;
    syscall!(socket(domain, socket_type, 0))
}
