use std::future::poll_fn;
use std::io;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::os::unix::net;
use std::path::Path;
use std::task::{ready, Context, Poll};

use super::UnixStream;
use crate::socket::{self, socketaddr::SocketAddr, Socket};

pub struct UnixListener {
    inner: socket::Listener,
}

impl UnixListener {
    pub fn bind<P>(path: P) -> io::Result<UnixListener>
    where
        P: AsRef<Path>,
    {
        Ok(UnixListener {
            inner: socket::Listener::bind_unix(path)?,
        })
    }

    pub async fn accept(&self) -> io::Result<(UnixStream, SocketAddr)> {
        poll_fn(|cx| self.poll_accept(cx)).await
    }

    pub async fn accept2(&self) -> io::Result<(UnixStream, SocketAddr)> {
        poll_fn(|cx| self.poll_accept2(cx)).await
    }

    pub fn poll_accept2(&self, cx: &mut Context<'_>) -> Poll<io::Result<(UnixStream, SocketAddr)>> {
        let socket = ready!(self.inner.poll_accept2(cx))?;
        let addr = SocketAddr::new(|sockaddr, socklen| {
            syscall!(getsockname(socket.as_raw_fd(), sockaddr, socklen))
        })?;
        Poll::Ready(Ok((socket.into(), addr)))
    }

    pub fn poll_accept(&self, cx: &mut Context<'_>) -> Poll<io::Result<(UnixStream, SocketAddr)>> {
        let (socket, socketstorage) = ready!(self.inner.poll_accept(cx))?;
        Poll::Ready(Ok((socket.into(), socketstorage.into())))
    }

    pub fn from_std(listener: net::UnixListener) -> io::Result<UnixListener> {
        Ok(UnixListener {
            inner: unsafe { socket::Listener::from_raw_fd(listener.into_raw_fd()) },
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        let socket = self.inner.as_raw_fd();
        SocketAddr::new(|sockaddr, socklen| syscall!(getsockname(socket, sockaddr, socklen)))
    }
}

impl AsRawFd for UnixListener {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.get_ref().as_raw_fd()
    }
}

impl FromRawFd for UnixListener {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        let socket = Socket::from_raw_fd(fd);
        UnixListener {
            inner: socket::Listener::new(socket),
        }
    }
}
