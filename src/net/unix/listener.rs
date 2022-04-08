use std::io;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::net;
use std::path::Path;

use super::UnixStream;
use crate::socket::{Socket, SocketAddr};

pub struct UnixListener {
    inner: Socket,
}

impl UnixListener {
    pub fn bind<P>(path: P) -> io::Result<UnixListener>
    where
        P: AsRef<Path>,
    {
        let socket = Socket::bind_unix(path, libc::SOCK_STREAM)?;
        socket.listen(1024)?;
        Ok(UnixListener { inner: socket })
    }

    pub async fn accept(&self) -> io::Result<(UnixStream, SocketAddr)> {
        let (socket, addr) = self.inner.accept_unix().await?;
        let stream: UnixStream = socket.into();
        Ok((stream, addr))
    }

    pub fn from_std(listener: net::UnixListener) -> io::Result<UnixListener> {
        let socket = unsafe { Socket::from_raw_fd(listener.into_raw_fd()) };
        Ok(UnixListener { inner: socket })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        let socket = self.inner.as_raw_fd();
        SocketAddr::new(|sockaddr, socklen| syscall!(getsockname(socket, sockaddr, socklen)))
    }
}
