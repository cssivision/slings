use std::io;
use std::net::{self, SocketAddr, ToSocketAddrs};
use std::os::unix::io::{AsRawFd, FromRawFd};

use super::stream::TcpStream;
use crate::io::action;

pub struct TcpListener {
    inner: net::TcpListener,
}

impl TcpListener {
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<TcpListener> {
        let listener = net::TcpListener::bind(addr)?;

        Ok(TcpListener { inner: listener })
    }

    pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let fd = self.inner.as_raw_fd();
        let accept = action::accept(fd)?;
        let (fd, addr) = accept.await?;

        let stream = unsafe { net::TcpStream::from_raw_fd(fd) };

        Ok((TcpStream::from_std(stream), addr))
    }
}
