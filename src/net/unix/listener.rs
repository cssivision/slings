use std::io;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::net;
use std::path::Path;

use super::UnixStream;
use crate::socket::{self, socketaddr::SocketAddr};

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
        let (socket, addr) = self.inner.accept_unix().await?;
        let stream: UnixStream = socket.into();
        Ok((stream, addr))
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
