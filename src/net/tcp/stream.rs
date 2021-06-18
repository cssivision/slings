use std::io;
use std::mem::ManuallyDrop;
use std::net::{self, SocketAddr, ToSocketAddrs};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures_util::io::{AsyncBufRead, AsyncRead, AsyncWrite};
use socket2::Socket;

use crate::driver::{self, Action};

pub struct TcpStream {
    inner: driver::Stream<net::TcpStream>,
}

impl FromRawFd for TcpStream {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        TcpStream::from_std(net::TcpStream::from_raw_fd(fd))
    }
}

impl TcpStream {
    pub fn from_std(stream: net::TcpStream) -> TcpStream {
        TcpStream {
            inner: driver::Stream::new(stream),
        }
    }

    async fn connect_addr(addr: SocketAddr) -> io::Result<TcpStream> {
        let completion = Action::connect(addr)?.await;
        let fd = completion.action.get_sock(completion.result)?;
        Ok(TcpStream::from_std(unsafe {
            net::TcpStream::from_raw_fd(fd)
        }))
    }

    pub async fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<TcpStream> {
        let addrs = addr.to_socket_addrs()?;

        let mut last_err = None;
        for addr in addrs {
            match TcpStream::connect_addr(addr).await {
                Ok(stream) => return Ok(stream),
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

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.get_ref().local_addr()
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.get_ref().peer_addr()
    }

    pub fn shutdown(&self, how: net::Shutdown) -> std::io::Result<()> {
        self.as_socket().shutdown(how)
    }

    pub fn nodelay(&self) -> io::Result<bool> {
        self.as_socket().nodelay()
    }

    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.as_socket().set_nodelay(nodelay)
    }

    pub fn set_keepalive(&self, keepalive: Option<Duration>) -> io::Result<()> {
        self.as_socket().set_keepalive(keepalive)
    }

    pub fn keepalive(&self) -> io::Result<Option<Duration>> {
        self.as_socket().keepalive()
    }

    fn as_socket(&self) -> ManuallyDrop<Socket> {
        let raw_fd = self.inner.get_ref().as_raw_fd();
        unsafe { ManuallyDrop::new(Socket::from_raw_fd(raw_fd)) }
    }
}

impl AsyncBufRead for TcpStream {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        self.get_mut().inner.poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        self.get_mut().inner.consume(amt);
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.get_mut().inner.poll_read(cx, buf)
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.get_mut().inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        self.get_mut().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context) -> Poll<io::Result<()>> {
        self.shutdown(net::Shutdown::Write)?;
        Poll::Ready(Ok(()))
    }
}
