use std::io;
use std::net::SocketAddr;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{socket_addr, Action};

pub struct Connect {
    fd: RawFd,
}

impl Action<Connect> {
    pub fn connect(addr: SocketAddr) -> io::Result<Action<Connect>> {
        let (sockaddr, socklen) = socket_addr(&addr);
        let fd = match addr {
            SocketAddr::V4(_) => new_v4_socket(),
            SocketAddr::V6(_) => new_v6_socket(),
        }?;

        let entry =
            opcode::Connect::new(types::Fd(fd), sockaddr.as_ptr() as *mut _, socklen).build();
        Action::submit(Connect { fd }, entry)
    }
}

impl Connect {
    pub fn get_sock(&self, result: io::Result<i32>) -> io::Result<RawFd> {
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
