use std::cell::RefCell;
use std::io;
use std::net::SocketAddr;
use std::os::unix::io::{AsRawFd, RawFd};
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::driver::{self, Action};

pub struct Packet<T> {
    inner: RefCell<Inner>,
    io: T,
}

impl<T: AsRawFd> Packet<T> {
    pub fn new(io: T) -> Packet<T> {
        Packet {
            io,
            inner: RefCell::new(Inner {
                recv: Recv::Idle,
                recv_from: RecvMsg::Idle,
                send: Send::Idle,
                send_to: SendMsg::Idle,
            }),
        }
    }

    pub fn get_ref(&self) -> &T {
        &self.io
    }

    pub fn poll_send(&self, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.inner
            .borrow_mut()
            .poll_send(cx, buf, self.io.as_raw_fd())
    }

    pub fn poll_send_to(
        &self,
        cx: &mut Context,
        buf: &[u8],
        addr: &SocketAddr,
    ) -> Poll<io::Result<usize>> {
        self.inner
            .borrow_mut()
            .poll_send_to(cx, buf, addr, self.io.as_raw_fd())
    }

    pub fn poll_recv(&self, cx: &mut Context, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        self.inner
            .borrow_mut()
            .poll_recv(cx, buf, self.io.as_raw_fd())
    }

    pub fn poll_recv_from(
        &self,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<io::Result<(usize, SocketAddr)>> {
        self.inner
            .borrow_mut()
            .poll_recv_from(cx, buf, self.io.as_raw_fd())
    }
}

struct Inner {
    recv: Recv,
    recv_from: RecvMsg,
    send: Send,
    send_to: SendMsg,
}

impl Inner {
    fn poll_send(&mut self, cx: &mut Context, buf: &[u8], fd: RawFd) -> Poll<io::Result<usize>> {
        loop {
            match &mut self.send {
                Send::Idle => {
                    let action = Action::send(fd, buf)?;
                    self.send = Send::Sending(action);
                }
                Send::Sending(action) => {
                    let n = ready!(Pin::new(action).poll_send(cx))?;
                    self.send = Send::Idle;
                    return Poll::Ready(Ok(n));
                }
            }
        }
    }

    fn poll_send_to(
        &mut self,
        cx: &mut Context,
        buf: &[u8],
        addr: &SocketAddr,
        fd: RawFd,
    ) -> Poll<io::Result<usize>> {
        loop {
            match &mut self.send_to {
                SendMsg::Idle => {
                    let action = Action::sendmsg(fd, buf, addr)?;
                    self.send_to = SendMsg::Sending(action);
                }
                SendMsg::Sending(action) => {
                    let n = ready!(Pin::new(action).poll_send_to(cx))?;
                    self.send_to = SendMsg::Idle;
                    return Poll::Ready(Ok(n));
                }
            }
        }
    }

    fn poll_recv(
        &mut self,
        cx: &mut Context,
        buf: &mut [u8],
        fd: RawFd,
    ) -> Poll<io::Result<usize>> {
        loop {
            match &mut self.recv {
                Recv::Idle => {
                    let action = Action::recv(fd, buf.len() as u32)?;
                    self.recv = Recv::Recving(action);
                }
                Recv::Recving(action) => {
                    let n = ready!(Pin::new(action).poll_recv(cx, buf))?;
                    self.recv = Recv::Idle;
                    return Poll::Ready(Ok(n));
                }
            }
        }
    }

    fn poll_recv_from(
        &mut self,
        cx: &mut Context,
        buf: &mut [u8],
        fd: RawFd,
    ) -> Poll<io::Result<(usize, SocketAddr)>> {
        loop {
            match &mut self.recv_from {
                RecvMsg::Idle => {
                    let action = Action::recvmsg(fd, buf.len())?;
                    self.recv_from = RecvMsg::Recving(action);
                }
                RecvMsg::Recving(action) => {
                    let res = ready!(Pin::new(action).poll_recv_from(cx, buf))?;
                    self.recv_from = RecvMsg::Idle;
                    return Poll::Ready(Ok(res));
                }
            }
        }
    }
}

enum Send {
    Idle,
    Sending(Action<driver::Send>),
}

enum SendMsg {
    Idle,
    Sending(Action<driver::SendMsg>),
}

enum Recv {
    Idle,
    Recving(Action<driver::Recv>),
}

enum RecvMsg {
    Idle,
    Recving(Action<driver::RecvMsg>),
}
