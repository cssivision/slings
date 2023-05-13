use std::future::poll_fn;
use std::io;
use std::net::{self, SocketAddr, ToSocketAddrs};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::task::{ready, Context, Poll};

use socket2::SockAddr;

use super::stream::TcpStream;
use crate::socket::{self, Socket};

pub struct TcpListener {
    inner: socket::Listener,
}

impl TcpListener {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<TcpListener> {
        let addrs = addr.to_socket_addrs()?;
        let mut last_err = None;

        for addr in addrs {
            match Self::bind_addr(addr) {
                Ok(v) => return Ok(v),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any address",
            )
        }))
    }

    fn bind_addr(addr: SocketAddr) -> io::Result<TcpListener> {
        Ok(TcpListener {
            inner: socket::Listener::bind(addr)?,
        })
    }

    pub fn from_std(listener: net::TcpListener) -> io::Result<TcpListener> {
        Ok(TcpListener {
            inner: unsafe { socket::Listener::from_raw_fd(listener.into_raw_fd()) },
        })
    }

    pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        poll_fn(|cx| self.poll_accept(cx)).await
    }

    pub async fn accept2(&self) -> io::Result<(TcpStream, SocketAddr)> {
        poll_fn(|cx| self.poll_accept2(cx)).await
    }

    pub fn poll_accept(&self, cx: &mut Context<'_>) -> Poll<io::Result<(TcpStream, SocketAddr)>> {
        let (socket, socketaddr) = ready!(self.inner.poll_accept(cx))?;
        let (_, addr) = unsafe {
            SockAddr::try_init(move |addr_storage, len| {
                *addr_storage = socketaddr.storage.to_owned();
                *len = socketaddr.socklen;
                Ok(())
            })?
        };
        let socket_addr = addr.as_socket().ok_or_else(|| {
            io::Error::new(io::ErrorKind::Other, "Could not get socket IP address")
        })?;
        Poll::Ready(Ok((socket.into(), socket_addr)))
    }

    pub fn poll_accept2(&self, cx: &mut Context<'_>) -> Poll<io::Result<(TcpStream, SocketAddr)>> {
        let socket = ready!(self.inner.poll_accept2(cx))?;
        let addr = socket.peer_addr()?;
        Poll::Ready(Ok((socket.into(), addr)))
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}

impl AsRawFd for TcpListener {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.get_ref().as_raw_fd()
    }
}

impl FromRawFd for TcpListener {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        let socket = Socket::from_raw_fd(fd);
        TcpListener {
            inner: socket::Listener::new(socket),
        }
    }
}
