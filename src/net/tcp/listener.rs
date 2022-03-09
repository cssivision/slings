use std::io;
use std::net::{self, SocketAddr};
use std::os::unix::io::{FromRawFd, IntoRawFd};

use super::stream::TcpStream;
use crate::driver::socket::Socket;

pub struct TcpListener {
    inner: Socket,
}

impl TcpListener {
    pub async fn bind(addr: SocketAddr) -> io::Result<TcpListener> {
        let socket = Socket::bind(addr, libc::SOCK_STREAM)?;
        socket.listen(1024)?;
        Ok(TcpListener { inner: socket })
    }

    pub fn from_std(listener: net::TcpListener) -> io::Result<TcpListener> {
        let socket = unsafe { Socket::from_raw_fd(listener.into_raw_fd()) };
        Ok(TcpListener { inner: socket })
    }

    pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let (socket, socket_addr) = self.inner.accept().await?;
        let socket_addr = socket_addr.ok_or_else(|| {
            io::Error::new(io::ErrorKind::Other, "Could not get socket IP address")
        })?;
        let stream: TcpStream = socket.into();
        Ok((stream, socket_addr))
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}
