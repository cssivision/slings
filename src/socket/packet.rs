use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use super::Socket;
use crate::driver::{self, Op};

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
                    self.send = SendState::Sending(Op::send(fd, buf)?);
                }
                SendState::Sending(op) => {
                    let n = ready!(Pin::new(op).poll(cx))?;
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
                    self.send_to = SendMsgState::Sending(Op::sendmsg(fd, buf, addr)?);
                }
                SendMsgState::Sending(op) => {
                    let n = ready!(Pin::new(op).poll(cx))?;
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
                    self.recv = RecvState::Recving(Op::recv(fd, buf.len())?);
                }
                RecvState::Recving(op) => {
                    let buf1 = ready!(Pin::new(op).poll(cx))?;
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
                    self.recv_from = RecvMsgState::Recving(Op::recvmsg(fd, buf.len())?);
                }
                RecvMsgState::Recving(op) => {
                    let (buf1, addr) = ready!(Pin::new(op).poll(cx))?;
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
    Sending(Op<driver::Send>),
}

enum SendMsgState {
    Idle,
    Sending(Op<driver::SendMsg>),
}

enum RecvState {
    Idle,
    Recving(Op<driver::Recv>),
}

enum RecvMsgState {
    Idle,
    Recving(Op<driver::RecvMsg>),
}
