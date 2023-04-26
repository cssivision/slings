use std::io;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::net;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_io::{AsyncBufRead, AsyncRead, AsyncWrite};
use socket2::SockAddr;

use crate::socket::{self, socketaddr::SocketAddr, Socket};

pub struct UnixStream {
    inner: socket::Stream,
}

impl UnixStream {
    pub async fn connect<P>(path: P) -> io::Result<UnixStream>
    where
        P: AsRef<Path>,
    {
        let socket = Socket::new_unix(libc::SOCK_STREAM)?;
        socket.connect(SockAddr::unix(path)?).await?;
        Ok(UnixStream {
            inner: socket::Stream::new(socket),
        })
    }

    pub fn from_std(stream: net::UnixStream) -> io::Result<UnixStream> {
        let socket = unsafe { Socket::from_raw_fd(stream.as_raw_fd()) };
        Ok(UnixStream {
            inner: socket::Stream::new(socket),
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        let socket = self.inner.get_ref().as_raw_fd();
        SocketAddr::new(|sockaddr, socklen| syscall!(getsockname(socket, sockaddr, socklen)))
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        let socket = self.inner.get_ref().as_raw_fd();
        SocketAddr::new(|sockaddr, socklen| syscall!(getpeername(socket, sockaddr, socklen)))
    }
}

impl AsyncRead for UnixStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.get_mut().inner.poll_read(cx, buf)
    }
}

impl AsyncBufRead for UnixStream {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        self.get_mut().inner.poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        self.get_mut().inner.consume(amt);
    }
}

impl AsyncWrite for UnixStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.get_mut().inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        self.get_mut()
            .inner
            .poll_shutdown(cx, std::net::Shutdown::Write)
    }
}

impl From<Socket> for UnixStream {
    fn from(socket: Socket) -> Self {
        UnixStream {
            inner: socket::Stream::new(socket),
        }
    }
}

impl AsRawFd for UnixStream {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.get_ref().as_raw_fd()
    }
}

impl FromRawFd for UnixStream {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        let socket = Socket::from_raw_fd(fd);
        UnixStream {
            inner: socket::Stream::new(socket),
        }
    }
}
