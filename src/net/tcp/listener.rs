use std::io;
use std::net::{self, SocketAddr, ToSocketAddrs};
use std::os::unix::io::{AsRawFd, FromRawFd};

use os_socketaddr::OsSocketAddr;

use super::stream::TcpStream;
use crate::driver::Action;

pub struct TcpListener {
    inner: net::TcpListener,
}

impl TcpListener {
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<TcpListener> {
        let listener = net::TcpListener::bind(addr)?;
        Ok(TcpListener { inner: listener })
    }

    pub fn from_std(listener: net::TcpListener) -> io::Result<TcpListener> {
        Ok(TcpListener { inner: listener })
    }

    pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let completion = Action::accept(self.inner.as_raw_fd())?.await;
        let fd = completion.result?;
        let stream = unsafe { TcpStream::from_raw_fd(fd) };
        let os_socket_addr = unsafe {
            OsSocketAddr::from_raw_parts(
                &completion.action.socketaddr.0 as *const _ as _,
                completion.action.socketaddr.1 as usize,
            )
        };
        let socket_addr = os_socket_addr.into_addr().unwrap();
        Ok((stream, socket_addr))
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}
