use std::io;
use std::mem::{size_of, MaybeUninit};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::unix::io::RawFd;

use io_uring::{cqueue, opcode, types};

use crate::driver::action::Completed;
use crate::driver::Action;

pub(crate) struct Accept {
    sockaddr: Box<MaybeUninit<libc::sockaddr_storage>>,
    result: i32,
}

impl Accept {
    pub fn get_socketaddr(&self) -> io::Result<SocketAddr> {
        unsafe { to_socket_addr(self.sockaddr.as_ptr()) }
    }
}

impl Drop for Accept {
    fn drop(&mut self) {
        if self.result >= 0 {
            let _sockaddr: libc::sockaddr_storage = unsafe { *self.sockaddr.as_ptr() };
        }
    }
}

impl Action<Accept> {
    pub(crate) fn accept(fd: RawFd) -> io::Result<Action<Accept>> {
        let mut sockaddr = Box::new(MaybeUninit::uninit());
        let mut length = size_of::<libc::sockaddr_storage>() as libc::socklen_t;
        let entry =
            opcode::Accept::new(types::Fd(fd), sockaddr.as_mut_ptr() as *mut _, &mut length)
                .flags(libc::SOCK_CLOEXEC)
                .build();

        Action::submit(
            Accept {
                sockaddr,
                result: 0,
            },
            entry,
        )
    }
}

impl Completed for Accept {
    fn completed(&mut self, cqe: &cqueue::Entry) {
        self.result = cqe.result();
    }
}

unsafe fn to_socket_addr(storage: *const libc::sockaddr_storage) -> io::Result<SocketAddr> {
    match (*storage).ss_family as libc::c_int {
        libc::AF_INET => {
            // Safety: if the ss_family field is AF_INET then storage must be a sockaddr_in.
            let addr: &libc::sockaddr_in = &*(storage as *const libc::sockaddr_in);
            let ip = Ipv4Addr::from(addr.sin_addr.s_addr.to_ne_bytes());
            let port = u16::from_be(addr.sin_port);
            Ok(SocketAddr::V4(SocketAddrV4::new(ip, port)))
        }
        libc::AF_INET6 => {
            // Safety: if the ss_family field is AF_INET6 then storage must be a sockaddr_in6.
            let addr: &libc::sockaddr_in6 = &*(storage as *const libc::sockaddr_in6);
            let ip = Ipv6Addr::from(addr.sin6_addr.s6_addr);
            let port = u16::from_be(addr.sin6_port);
            Ok(SocketAddr::V6(SocketAddrV6::new(
                ip,
                port,
                addr.sin6_flowinfo,
                addr.sin6_scope_id,
            )))
        }
        _ => Err(io::ErrorKind::InvalidInput.into()),
    }
}
