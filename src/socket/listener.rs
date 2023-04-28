#![allow(clippy::type_complexity)]
use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::path::Path;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use futures_core::stream::Stream;

use super::Socket;
use crate::driver::{self, Op};

pub(crate) struct Listener {
    inner: RefCell<Inner>,
    io: Socket,
}

impl Listener {
    pub(crate) fn new(io: Socket) -> Listener {
        Listener {
            io,
            inner: RefCell::new(Inner {
                accept: AcceptState::Idle,
            }),
        }
    }

    pub(crate) fn get_ref(&self) -> &Socket {
        &self.io
    }

    pub(crate) fn bind(addr: SocketAddr) -> io::Result<Listener> {
        let socket = Socket::bind(addr, libc::SOCK_STREAM)?;
        socket.listen(1024)?;
        Ok(Listener::new(socket))
    }

    pub(crate) fn bind_unix<P>(path: P) -> io::Result<Listener>
    where
        P: AsRef<Path>,
    {
        let socket = Socket::bind_unix(path, libc::SOCK_STREAM)?;
        socket.listen(1024)?;
        Ok(Listener::new(socket))
    }

    pub(crate) fn accept_multi(&self) -> AcceptMulti {
        AcceptMulti {
            fd: self.io.as_raw_fd(),
            state: AcceptMultiState::Idle,
        }
    }

    pub(crate) fn poll_accept(
        &self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<(Socket, Box<(libc::sockaddr_storage, libc::socklen_t)>)>> {
        self.inner.borrow_mut().poll_accept(cx, self.io.as_raw_fd())
    }

    pub(crate) fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.local_addr()
    }
}

impl FromRawFd for Listener {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Listener::new(Socket { fd })
    }
}

impl AsRawFd for Listener {
    fn as_raw_fd(&self) -> RawFd {
        self.io.as_raw_fd()
    }
}

struct Inner {
    accept: AcceptState,
}

enum AcceptState {
    Idle,
    Accepting(Op<driver::Accept>),
}

impl Inner {
    pub fn poll_accept(
        &mut self,
        cx: &mut Context<'_>,
        fd: RawFd,
    ) -> Poll<io::Result<(Socket, Box<(libc::sockaddr_storage, libc::socklen_t)>)>> {
        loop {
            match &mut self.accept {
                AcceptState::Idle => {
                    self.accept = AcceptState::Accepting(Op::accept(fd)?);
                }
                AcceptState::Accepting(op) => {
                    let (socket, socketaddr) = ready!(Pin::new(op).poll(cx))?;
                    self.accept = AcceptState::Idle;
                    return Poll::Ready(Ok((socket, socketaddr)));
                }
            }
        }
    }
}

pub(crate) struct AcceptMulti {
    fd: RawFd,
    state: AcceptMultiState,
}

enum AcceptMultiState {
    Idle,
    Accepting(Op<driver::AcceptMulti>),
    Done,
}

impl Stream for AcceptMulti {
    type Item = io::Result<(Socket, SocketAddr)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match &mut self.state {
                AcceptMultiState::Idle => {
                    self.state = AcceptMultiState::Accepting(Op::accept_multi(self.fd)?);
                }
                AcceptMultiState::Accepting(op) => {
                    if let Some(res) = op.get_mut().next() {
                        let socket = unsafe { Socket::from_raw_fd(res.result? as i32) };
                        let socket_addr = socket.peer_addr()?;
                        return Poll::Ready(Some(Ok((socket, socket_addr))));
                    }
                    let res = ready!(Pin::new(op).poll(cx));
                    let socket = unsafe { Socket::from_raw_fd(res.result? as i32) };
                    let socket_addr = socket.peer_addr()?;
                    self.state = AcceptMultiState::Done;
                    return Poll::Ready(Some(Ok((socket, socket_addr))));
                }
                AcceptMultiState::Done => {
                    return Poll::Ready(None);
                }
            }
        }
    }
}