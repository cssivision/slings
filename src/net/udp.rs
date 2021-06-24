use std::io;
use std::net::{self, SocketAddr, ToSocketAddrs};
use std::os::unix::io::AsRawFd;

use futures_util::future::poll_fn;

use crate::driver::Action;

#[derive(Debug)]
pub struct UdpSocket {
    inner: net::UdpSocket,
}

impl UdpSocket {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<UdpSocket> {
        let addrs = addr.to_socket_addrs()?;
        let mut last_err = None;

        for addr in addrs {
            match UdpSocket::bind_addr(addr) {
                Ok(socket) => return Ok(socket),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any address",
            )
        }))
    }

    fn bind_addr(addr: SocketAddr) -> io::Result<UdpSocket> {
        Ok(UdpSocket {
            inner: net::UdpSocket::bind(addr)?,
        })
    }

    pub fn connect<A: ToSocketAddrs>(&self, addr: A) -> io::Result<()> {
        let addrs = addr.to_socket_addrs()?;
        let mut last_err = None;

        for addr in addrs {
            match self.inner.connect(addr) {
                Ok(_) => return Ok(()),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any address",
            )
        }))
    }

    pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        let mut action = Action::recv(self.inner.as_raw_fd(), buf.len() as _)?;
        poll_fn(|cx| action.poll_recv(cx, buf)).await
    }

    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        let mut action = Action::send(self.inner.as_raw_fd(), buf)?;
        poll_fn(|cx| action.poll_send(cx)).await
    }

    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        let mut action = Action::recvmsg(self.inner.as_raw_fd(), buf.len())?;
        poll_fn(|cx| action.poll_recv_from(cx, buf)).await
    }

    pub async fn send_to<A: Into<SocketAddr>>(&self, buf: &[u8], target: A) -> io::Result<usize> {
        let addr = target.into();
        let mut action = Action::sendmsg(self.inner.as_raw_fd(), buf, &addr)?;
        poll_fn(|cx| action.poll_send_to(cx)).await
    }
}
