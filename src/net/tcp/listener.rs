use std::io;
use std::net::{self, SocketAddr, ToSocketAddrs};
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
        let addr = completion.action.get_socketaddr()?;
        Ok((unsafe { TcpStream::from_raw_fd(fd) }, addr))
    }
}
