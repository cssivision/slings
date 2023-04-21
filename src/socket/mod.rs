pub(crate) mod packet;
pub(crate) mod socketaddr;
pub(crate) mod stream;

pub(crate) use packet::Packet;
pub(crate) use stream::Stream;

use std::io;
use std::mem;
use std::net::SocketAddr;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::path::Path;

use crate::driver::Action;

use socket2::SockAddr;

pub(crate) struct Socket {
    pub(crate) fd: RawFd,
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
        Ok(Socket { fd })
    }

    pub(crate) fn new_unix(socket_type: libc::c_int) -> io::Result<Socket> {
        let socket_type = socket_type | libc::SOCK_CLOEXEC;
        let domain = libc::AF_UNIX;
        let fd = socket2::Socket::new(domain.into(), socket_type.into(), None)?.into_raw_fd();
        Ok(Socket { fd })
    }

    pub(crate) async fn connect(&self, sock_addr: SockAddr) -> io::Result<()> {
        Action::connect(self.fd, sock_addr)?.await
    }

    pub(crate) fn bind(socket_addr: SocketAddr, socket_type: libc::c_int) -> io::Result<Socket> {
        Self::bind_internal(
            socket_addr.into(),
            get_domain(socket_addr).into(),
            socket_type.into(),
        )
    }

    pub(crate) fn bind_unix<P: AsRef<Path>>(
        path: P,
        socket_type: libc::c_int,
    ) -> io::Result<Socket> {
        let addr = socket2::SockAddr::unix(path.as_ref())?;
        Socket::bind_internal(addr, libc::AF_UNIX.into(), socket_type.into())
    }

    fn bind_internal(
        socket_addr: socket2::SockAddr,
        domain: socket2::Domain,
        socket_type: socket2::Type,
    ) -> io::Result<Socket> {
        let sys_listener = socket2::Socket::new(domain, socket_type, None)?;
        sys_listener.set_reuse_port(true)?;
        sys_listener.set_reuse_address(true)?;
        sys_listener.bind(&socket_addr)?;
        let fd = sys_listener.into_raw_fd();
        Ok(Self { fd })
    }

    pub(crate) fn listen(&self, backlog: libc::c_int) -> io::Result<()> {
        syscall!(listen(self.as_raw_fd(), backlog))?;
        Ok(())
    }

    pub(crate) async fn accept(&self) -> io::Result<(Socket, Option<SocketAddr>)> {
        Action::accept(self.fd)?.await
    }

    pub(crate) async fn accept_unix(&self) -> io::Result<(Socket, socketaddr::SocketAddr)> {
        Action::accept_unix(self.fd)?.await
    }

    pub(crate) fn local_addr(&self) -> io::Result<SocketAddr> {
        sockname(|buf, len| syscall!(getsockname(self.as_raw_fd(), buf, len)))
    }

    pub(crate) fn peer_addr(&self) -> io::Result<SocketAddr> {
        sockname(|buf, len| syscall!(getpeername(self.as_raw_fd(), buf, len)))
    }

    pub(crate) fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        setsockopt(
            self.as_raw_fd(),
            libc::IPPROTO_TCP,
            libc::TCP_NODELAY,
            nodelay as libc::c_int,
        )
    }
}

fn setsockopt<T>(
    sock: libc::c_int,
    opt: libc::c_int,
    val: libc::c_int,
    payload: T,
) -> io::Result<()> {
    let payload = &payload as *const T as *const libc::c_void;
    syscall!(setsockopt(
        sock,
        opt,
        val,
        payload,
        mem::size_of::<T>() as libc::socklen_t,
    ))?;
    Ok(())
}

fn sockname<F>(f: F) -> io::Result<SocketAddr>
where
    F: FnOnce(*mut libc::sockaddr, *mut libc::socklen_t) -> io::Result<libc::c_int>,
{
    let mut storage: libc::sockaddr_storage = unsafe { mem::zeroed() };
    let mut len = mem::size_of_val(&storage) as libc::socklen_t;
    f(&mut storage as *mut _ as *mut _, &mut len)?;
    let (_, addr) = unsafe {
        SockAddr::try_init(move |addr_storage, length| {
            *addr_storage = storage.to_owned();
            *length = len;
            Ok(())
        })?
    };
    addr.as_socket()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid argument"))
}

impl Drop for Socket {
    fn drop(&mut self) {
        let _ = unsafe { libc::close(self.fd) };
    }
}

impl AsRawFd for Socket {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl FromRawFd for Socket {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Socket { fd }
    }
}
