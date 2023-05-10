use std::future::poll_fn;
use std::io;
use std::net::{self, SocketAddr, ToSocketAddrs};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use futures_core::stream::Stream;
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

    pub fn accept_multi(&self) -> impl Stream<Item = io::Result<(TcpStream, SocketAddr)>> {
        AcceptMulti {
            inner: self.inner.accept_multi(),
        }
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

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}

struct AcceptMulti {
    inner: socket::AcceptMulti,
}

impl Stream for AcceptMulti {
    type Item = io::Result<(TcpStream, SocketAddr)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match ready!(Pin::new(&mut self.inner).poll_next(cx)) {
            Some(item) => {
                let socket = item?;
                let socket_addr = socket.peer_addr()?;
                Poll::Ready(Some(Ok((socket.into(), socket_addr))))
            }
            None => Poll::Ready(None),
        }
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
