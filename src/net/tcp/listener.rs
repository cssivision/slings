use std::io;
use std::net::{self, SocketAddr, ToSocketAddrs};
use std::os::unix::io::AsRawFd;

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

        let (stream, addr) = accept.await?;
        Ok((TcpStream::from_std(stream), addr))
    }
}
