use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use super::Socket;
use crate::driver::{self, Action};

pub(crate) struct Packet {
    inner: RefCell<Inner>,
    io: Socket,
}

impl Packet {
    pub(crate) fn new(io: Socket) -> Packet {
        Packet {
            io,
            inner: RefCell::new(Inner {
                recv: RecvState::Idle,
                recv_from: RecvMsgState::Idle,
                send: SendState::Idle,
                send_to: SendMsgState::Idle,
            }),
        }
    }

    pub(crate) fn get_ref(&self) -> &Socket {
        &self.io
    }

    pub(crate) fn poll_send(&self, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.inner.borrow_mut().poll_send(cx, buf, self.io.fd)
    }

    pub(crate) fn poll_send_to(
        &self,
        cx: &mut Context,
        buf: &[u8],
        addr: SocketAddr,
    ) -> Poll<io::Result<usize>> {
        self.inner
            .borrow_mut()
            .poll_send_to(cx, buf, addr, self.io.fd)
    }

    pub(crate) fn poll_recv(&self, cx: &mut Context, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        self.inner.borrow_mut().poll_recv(cx, buf, self.io.fd)
    }

    pub(crate) fn poll_recv_from(
        &self,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<io::Result<(usize, SocketAddr)>> {
        self.inner.borrow_mut().poll_recv_from(cx, buf, self.io.fd)
    }
}

struct Inner {
    recv: RecvState,
    recv_from: RecvMsgState,
    send: SendState,
    send_to: SendMsgState,
}

impl Inner {
    fn poll_send(&mut self, cx: &mut Context, buf: &[u8], fd: RawFd) -> Poll<io::Result<usize>> {
        loop {
            match &mut self.send {
                SendState::Idle => {
                    let action = Action::send(fd, buf)?;
                    self.send = SendState::Sending(action);
                }
                SendState::Sending(action) => {
                    let n = ready!(Pin::new(action).poll(cx))?;
                    self.send = SendState::Idle;
                    return Poll::Ready(Ok(n));
                }
            }
        }
    }

    fn poll_send_to(
        &mut self,
        cx: &mut Context,
        buf: &[u8],
        addr: SocketAddr,
        fd: RawFd,
    ) -> Poll<io::Result<usize>> {
        loop {
            match &mut self.send_to {
                SendMsgState::Idle => {
                    let action = Action::sendmsg(fd, buf, addr)?;
                    self.send_to = SendMsgState::Sending(action);
                }
                SendMsgState::Sending(action) => {
                    let n = ready!(Pin::new(action).poll(cx))?;
                    self.send_to = SendMsgState::Idle;
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
                RecvState::Idle => {
                    let action = Action::recv(fd, buf.len())?;
                    self.recv = RecvState::Recving(action);
                }
                RecvState::Recving(action) => {
                    let buf1 = ready!(Pin::new(action).poll(cx))?;
                    let n = buf1.len();
                    buf[..n].copy_from_slice(&buf1[..n]);
                    self.recv = RecvState::Idle;
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
                RecvMsgState::Idle => {
                    let action = Action::recvmsg(fd, buf.len())?;
                    self.recv_from = RecvMsgState::Recving(action);
                }
                RecvMsgState::Recving(action) => {
                    let (buf1, addr) = ready!(Pin::new(action).poll(cx))?;
                    let n = buf1.len();
                    buf[..n].copy_from_slice(&buf1[..n]);
                    self.recv_from = RecvMsgState::Idle;
                    return Poll::Ready(Ok((n, addr)));
                }
            }
        }
    }
}

enum SendState {
    Idle,
    Sending(Action<driver::Send>),
}

enum SendMsgState {
    Idle,
    Sending(Action<driver::SendMsg>),
}

enum RecvState {
    Idle,
    Recving(Action<driver::Recv>),
}

enum RecvMsgState {
    Idle,
    Recving(Action<driver::RecvMsg>),
}
