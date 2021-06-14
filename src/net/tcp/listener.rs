use std::io;
use std::net::{self, SocketAddr, ToSocketAddrs};
use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};

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

    pub async fn accept(&self) -> io::Result<()> {
        let action = Action::accept(self.inner)?;
        let completion = action.await;
        let fd = completion.result?;
        Ok(())
    }
}
