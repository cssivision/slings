use std::io;
use std::mem::size_of;
use std::net::SocketAddr;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::Action;

pub(crate) struct Connect {
    fd: RawFd,
}

impl Action<Connect> {
    pub(crate) fn connect(addr: SocketAddr) -> io::Result<Action<Connect>> {
        let (sockaddr, socklen) = socket_addr(&addr);
        let fd = match addr {
            SocketAddr::V4(_) => new_v4_socket(),
            SocketAddr::V6(_) => new_v6_socket(),
        }?;

        let entry = opcode::Read::new(types::Fd(fd), sockaddr.as_ptr() as *mut _, socklen).build();

        Action::submit(Connect { fd }, entry)
    }
}

impl Connect {
    pub(crate) fn get_sock(&self, result: io::Result<i32>) -> io::Result<RawFd> {
        match result {
            Err(err) if err.raw_os_error() != Some(libc::EINPROGRESS) => Err(err),
            _ => Ok(self.fd),
        }
    }
}

pub(crate) fn new_v4_socket() -> io::Result<i32> {
    new_socket(libc::AF_INET, libc::SOCK_STREAM)
}

pub(crate) fn new_v6_socket() -> io::Result<i32> {
    new_socket(libc::AF_INET6, libc::SOCK_STREAM)
}

pub(crate) fn new_socket(domain: libc::c_int, socket_type: libc::c_int) -> io::Result<libc::c_int> {
    let socket_type = socket_type | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC;
    let socket = syscall!(socket(domain, socket_type, 0));
    socket
}

#[repr(C)]
union SockAddrIn {
    v4: libc::sockaddr_in,
    v6: libc::sockaddr_in6,
}

impl SockAddrIn {
    fn as_ptr(&self) -> *const libc::sockaddr {
        self as *const _ as *const libc::sockaddr
    }
}

fn socket_addr(addr: &SocketAddr) -> (SockAddrIn, libc::socklen_t) {
    match addr {
        SocketAddr::V4(ref addr) => {
            // `s_addr` is stored as BE on all machine and the array is in BE order.
            // So the native endian conversion method is used so that it's never swapped.
            let sin_addr = libc::in_addr {
                s_addr: u32::from_ne_bytes(addr.ip().octets()),
            };

            let sockaddr_in = libc::sockaddr_in {
                sin_family: libc::AF_INET as libc::sa_family_t,
                sin_port: addr.port().to_be(),
                sin_addr,
                sin_zero: [0; 8],
            };

            let sockaddr = SockAddrIn { v4: sockaddr_in };
            let socklen = size_of::<libc::sockaddr_in>() as libc::socklen_t;
            (sockaddr, socklen)
        }
        SocketAddr::V6(ref addr) => {
            let sockaddr_in6 = libc::sockaddr_in6 {
                sin6_family: libc::AF_INET6 as libc::sa_family_t,
                sin6_port: addr.port().to_be(),
                sin6_addr: libc::in6_addr {
                    s6_addr: addr.ip().octets(),
                },
                sin6_flowinfo: addr.flowinfo(),
                sin6_scope_id: addr.scope_id(),
            };

            let sockaddr = SockAddrIn { v6: sockaddr_in6 };
            let socklen = size_of::<libc::sockaddr_in6>() as libc::socklen_t;
            (sockaddr, socklen)
        }
    }
}
