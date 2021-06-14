use std::io;
use std::net::{self, SocketAddr, ToSocketAddrs};
use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};

use crate::driver::Action;
use crate::net::to_socket_addr;

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

    pub async fn accept(&self) -> io::Result<(net::TcpStream, SocketAddr)> {
        let action = Action::accept(self.inner)?;
        let completion = action.await;
        let fd = completion.result?;
        let addr = unsafe { to_socket_addr(completion.action.addr.as_ptr()) }?;
        Ok((unsafe { net::TcpStream::from_raw_fd(fd) }, addr))
    }
}
