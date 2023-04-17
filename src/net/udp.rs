use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::task::{Context, Poll};

use socket2::SockAddr;

use crate::future::poll_fn;
use crate::socket::{Packet, Socket};

pub struct UdpSocket {
    inner: Packet,
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
        let socket = Socket::bind(addr, libc::SOCK_DGRAM)?;
        Ok(UdpSocket {
            inner: Packet::new(socket),
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.get_ref().local_addr()
    }

    pub async fn connect<A: ToSocketAddrs>(&self, addr: A) -> io::Result<()> {
        let addrs = addr.to_socket_addrs()?;
        let mut last_err = None;

        for addr in addrs {
            match self.inner.get_ref().connect(SockAddr::from(addr)).await {
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
        poll_fn(|cx| self.inner.poll_recv(cx, buf)).await
    }

    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        poll_fn(|cx| self.inner.poll_send(cx, buf)).await
    }

    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        poll_fn(|cx| self.inner.poll_recv_from(cx, buf)).await
    }

    pub fn poll_recv_from(
        &self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<(usize, SocketAddr)>> {
        self.inner.poll_recv_from(cx, buf)
    }

    pub async fn send_to<A: Into<SocketAddr>>(&self, buf: &[u8], target: A) -> io::Result<usize> {
        let addr = target.into();
        poll_fn(|cx| self.inner.poll_send_to(cx, buf, addr)).await
    }

    pub fn poll_send_to<A: Into<SocketAddr>>(
        &self,
        cx: &mut Context<'_>,
        buf: &[u8],
        target: A,
    ) -> Poll<io::Result<usize>> {
        let addr = target.into();
        self.inner.poll_send_to(cx, buf, addr)
    }
}
