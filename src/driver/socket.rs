use std::io;
use std::mem;
use std::net;
use std::net::SocketAddr;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use crate::driver::shared_fd::SharedFd;
use crate::driver::Action;

use os_socketaddr::OsSocketAddr;

#[derive(Clone)]
pub struct Socket {
    pub(crate) fd: SharedFd,
}

fn get_domain(socket_addr: SocketAddr) -> libc::c_int {
    match socket_addr {
        SocketAddr::V4(_) => libc::AF_INET,
        SocketAddr::V6(_) => libc::AF_INET6,
    }
}

impl Socket {
    pub(crate) fn new(socket_addr: SocketAddr, socket_type: libc::c_int) -> io::Result<Socket> {
        let socket_type = socket_type | libc::SOCK_CLOEXEC;
        let domain = get_domain(socket_addr);
        let fd = socket2::Socket::new(domain.into(), socket_type.into(), None)?.into_raw_fd();
        let fd = SharedFd::new(fd);
        Ok(Socket { fd })
    }

    pub(crate) async fn connect(&self, socket_addr: SocketAddr) -> io::Result<()> {
        let action = Action::connect(&self.fd, socket_addr)?;
        let completion = action.await;
        completion.result?;
        Ok(())
    }

    pub(crate) fn bind(socket_addr: SocketAddr, socket_type: libc::c_int) -> io::Result<Socket> {
        Self::bind_internal(
            socket_addr.into(),
            get_domain(socket_addr).into(),
            socket_type.into(),
        )
    }

    fn bind_internal(
        socket_addr: socket2::SockAddr,
        domain: socket2::Domain,
        socket_type: socket2::Type,
    ) -> io::Result<Socket> {
        let sys_listener = socket2::Socket::new(domain, socket_type, None)?;
        let addr = socket2::SockAddr::from(socket_addr);
        sys_listener.set_reuse_port(true)?;
        sys_listener.set_reuse_address(true)?;
        sys_listener.bind(&addr)?;
        let fd = SharedFd::new(sys_listener.into_raw_fd());
        Ok(Self { fd })
    }

    pub(crate) fn listen(&self, backlog: libc::c_int) -> io::Result<()> {
        syscall!(listen(self.as_raw_fd(), backlog))?;
        Ok(())
    }

    pub(crate) async fn accept(&self) -> io::Result<(Socket, Option<SocketAddr>)> {
        let completion = Action::accept(&self.fd)?.await;
        let fd = completion.result?;
        let fd = SharedFd::new(fd as i32);
        let socket = Socket { fd };
        let os_socket_addr = unsafe {
            OsSocketAddr::from_raw_parts(
                &completion.action.socketaddr.0 as *const _ as _,
                completion.action.socketaddr.1 as usize,
            )
        };
        let socket_addr = os_socket_addr.into_addr();
        Ok((socket, socket_addr))
    }

    pub(crate) fn local_addr(&self) -> io::Result<SocketAddr> {
        sockname(|buf, len| unsafe { libc::getsockname(self.as_raw_fd(), buf, len) })
    }

    pub(crate) fn peer_addr(&self) -> io::Result<SocketAddr> {
        sockname(|buf, len| unsafe { libc::getpeername(self.as_raw_fd(), buf, len) })
    }

    pub fn shutdown(&self, how: net::Shutdown) -> std::io::Result<()> {
        let how = match how {
            net::Shutdown::Write => libc::SHUT_WR,
            net::Shutdown::Read => libc::SHUT_RD,
            net::Shutdown::Both => libc::SHUT_RDWR,
        };
        syscall!(shutdown(self.as_raw_fd(), how))?;
        Ok(())
    }
}

fn sockname<F>(f: F) -> io::Result<SocketAddr>
where
    F: FnOnce(*mut libc::sockaddr, *mut libc::socklen_t) -> libc::c_int,
{
    unsafe {
        let mut storage: libc::sockaddr_storage = mem::zeroed();
        let mut len = mem::size_of_val(&storage) as libc::socklen_t;
        let res = f(&mut storage as *mut _ as *mut _, &mut len);
        if res == -1 {
            return Err(std::io::Error::last_os_error());
        }
        let os_socket_addr = OsSocketAddr::from_raw_parts(&storage as *const _ as _, len as usize);
        os_socket_addr
            .into_addr()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid argument"))
    }
}

impl AsRawFd for Socket {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.raw_fd()
    }
}

impl FromRawFd for Socket {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Socket {
            fd: SharedFd::new(fd),
        }
    }
}