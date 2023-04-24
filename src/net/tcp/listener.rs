use std::io;
use std::net::{self, SocketAddr, ToSocketAddrs};
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use futures_core::stream::Stream;

use super::stream::TcpStream;
use crate::socket;

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
        let (socket, socket_addr) = self.inner.accept().await?;
        Ok((socket.into(), socket_addr))
    }

    pub fn accept_multi(&self) -> AcceptMulti {
        AcceptMulti {
            inner: self.inner.accept_multi(),
        }
    }

    pub fn poll_accept(&self, cx: &mut Context<'_>) -> Poll<io::Result<(TcpStream, SocketAddr)>> {
        let (socket, socket_addr) = ready!(self.inner.poll_accept(cx))?;
        Poll::Ready(Ok((socket.into(), socket_addr)))
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}

pub struct AcceptMulti {
    inner: socket::AcceptMulti,
}

impl Stream for AcceptMulti {
    type Item = io::Result<(TcpStream, SocketAddr)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match ready!(Pin::new(&mut self.inner).poll_next(cx)) {
            Some(item) => {
                let (socket, socket_addr) = item?;
                Poll::Ready(Some(Ok((socket.into(), socket_addr))))
            }
            None => Poll::Ready(None),
        }
    }
}
