use std::io;
use std::net::{self, SocketAddr, ToSocketAddrs};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::io::{AsyncBufRead, AsyncRead, AsyncWrite};
use socket2::SockAddr;

use crate::socket::{self, Socket};

pub struct TcpStream {
    inner: socket::Stream,
}

impl FromRawFd for TcpStream {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        let socket = Socket::from_raw_fd(fd);
        TcpStream {
            inner: socket::Stream::new(socket),
        }
    }
}

impl TcpStream {
    pub fn from_std(stream: net::TcpStream) -> TcpStream {
        let socket = unsafe { Socket::from_raw_fd(stream.as_raw_fd()) };
        TcpStream {
            inner: socket::Stream::new(socket),
        }
    }

    async fn connect_addr(addr: SocketAddr) -> io::Result<TcpStream> {
        let socket = Socket::new(addr, libc::SOCK_STREAM)?;
        socket.connect(SockAddr::from(addr)).await?;
        Ok(TcpStream {
            inner: socket::Stream::new(socket),
        })
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

    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.inner.get_ref().set_nodelay(nodelay)
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

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        self.get_mut().inner.poll_shutdown(cx, net::Shutdown::Write)
    }
}

impl From<Socket> for TcpStream {
    fn from(socket: Socket) -> Self {
        TcpStream {
            inner: socket::Stream::new(socket),
        }
    }
}
