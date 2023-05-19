use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::path::Path;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use super::{Socket, SocketStorage};
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
                accept_multi: AcceptMultiState::Idle,
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

    pub(crate) fn poll_accept(
        &self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<(Socket, SocketStorage)>> {
        self.inner.borrow_mut().poll_accept(cx, self.io.as_raw_fd())
    }

    pub(crate) fn poll_accept2(&self, cx: &mut Context<'_>) -> Poll<io::Result<Socket>> {
        self.inner
            .borrow_mut()
            .poll_accept2(cx, self.io.as_raw_fd())
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
    accept_multi: AcceptMultiState,
}

enum AcceptState {
    Idle,
    Accepting(Op<driver::Accept>),
}

enum AcceptMultiState {
    Idle,
    Accepting(Op<driver::AcceptMulti>),
}

impl Inner {
    pub fn poll_accept(
        &mut self,
        cx: &mut Context<'_>,
        fd: RawFd,
    ) -> Poll<io::Result<(Socket, SocketStorage)>> {
        loop {
            match &mut self.accept {
                AcceptState::Idle => {
                    self.accept = AcceptState::Accepting(Op::accept(fd)?);
                }
                AcceptState::Accepting(op) => {
                    let (socket, socketaddr) = ready!(Pin::new(op).poll(cx))?;
                    self.accept = AcceptState::Idle;
                    return Poll::Ready(Ok((
                        socket,
                        SocketStorage {
                            storage: socketaddr.0,
                            socklen: socketaddr.1,
                        },
                    )));
                }
            }
        }
    }

    pub fn poll_accept2(&mut self, cx: &mut Context<'_>, fd: RawFd) -> Poll<io::Result<Socket>> {
        loop {
            match &mut self.accept_multi {
                AcceptMultiState::Idle => {
                    self.accept_multi = AcceptMultiState::Accepting(Op::accept_multi(fd)?);
                }
                AcceptMultiState::Accepting(op) => {
                    if let Some(fd) = op.get_mut().next() {
                        let fd = fd?;
                        let socket = unsafe { Socket::from_raw_fd(fd) };
                        return Poll::Ready(Ok(socket));
                    }
                    let fd = ready!(Pin::new(op).poll(cx))?;
                    let socket = unsafe { Socket::from_raw_fd(fd) };
                    self.accept_multi = AcceptMultiState::Idle;
                    return Poll::Ready(Ok(socket));
                }
            }
        }
    }
}
