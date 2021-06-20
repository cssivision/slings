use std::io;
use std::net::{self, Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs};
use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};

use super::stream::TcpStream;
use crate::driver::Action;

pub struct TcpListener {
    inner: RawFd,
}

impl TcpListener {
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<TcpListener> {
        let listener = net::TcpListener::bind(addr)?;

        Ok(TcpListener {
            inner: listener.into_raw_fd(),
        })
    }

    pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let completion = Action::accept(self.inner)?.await;
        let fd = completion.result?;
        let stream = unsafe { TcpStream::from_raw_fd(fd) };
        let addr = stream
            .peer_addr()
            .unwrap_or_else(|_| SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)));
        Ok((stream, addr))
    }
}
