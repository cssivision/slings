use std::cell::RefCell;
use std::future::{poll_fn, Future};
use std::io;
use std::net::SocketAddr;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::path::Path;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use futures_core::stream::Stream;

use super::Socket;
use crate::driver::{self, Action};
use crate::socket::socketaddr;

pub(crate) struct Listener {
    inner: RefCell<Inner>,
    io: Socket,
}

impl AsRawFd for Listener {
    fn as_raw_fd(&self) -> RawFd {
        self.io.as_raw_fd()
    }
}

impl Listener {
    pub(crate) fn new(io: Socket) -> Listener {
        Listener {
            io,
            inner: RefCell::new(Inner {
                accept: AcceptState::Idle,
                accept_unix: AcceptUnixState::Idle,
            }),
        }
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

    pub(crate) async fn accept(&self) -> io::Result<(Socket, SocketAddr)> {
        poll_fn(|cx| self.inner.borrow_mut().poll_accept(cx, self.io.as_raw_fd())).await
    }

    pub(crate) async fn accept_unix(&self) -> io::Result<(Socket, socketaddr::SocketAddr)> {
        poll_fn(|cx| {
            self.inner
                .borrow_mut()
                .poll_accept_unix(cx, self.io.as_raw_fd())
        })
        .await
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
    ) -> Poll<io::Result<(Socket, SocketAddr)>> {
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

impl Inner {
    pub fn poll_accept(
        &mut self,
        cx: &mut Context<'_>,
        fd: RawFd,
    ) -> Poll<io::Result<(Socket, SocketAddr)>> {
        loop {
            match &mut self.accept {
                AcceptState::Idle => {
                    self.accept = AcceptState::Accepting(Action::accept(fd)?);
                }
                AcceptState::Accepting(action) => {
                    let (socket, socket_addr) = ready!(Pin::new(action).poll(cx))?;
                    let socket_addr = socket_addr.ok_or_else(|| {
                        io::Error::new(io::ErrorKind::Other, "Could not get socket IP address")
                    })?;
                    self.accept = AcceptState::Idle;
                    return Poll::Ready(Ok((socket, socket_addr)));
                }
            }
        }
    }

    pub fn poll_accept_unix(
        &mut self,
        cx: &mut Context<'_>,
        fd: RawFd,
    ) -> Poll<io::Result<(Socket, socketaddr::SocketAddr)>> {
        loop {
            match &mut self.accept_unix {
                AcceptUnixState::Idle => {
                    self.accept_unix = AcceptUnixState::Accepting(Action::accept_unix(fd)?);
                }
                AcceptUnixState::Accepting(action) => {
                    let (socket, socket_addr) = ready!(Pin::new(action).poll(cx))?;
                    self.accept = AcceptState::Idle;
                    return Poll::Ready(Ok((socket, socket_addr)));
                }
            }
        }
    }
}

impl Stream for AcceptMulti {
    type Item = io::Result<(Socket, SocketAddr)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_accept_multi(cx)
    }
}

impl AcceptMulti {
    fn poll_accept_multi(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<io::Result<(Socket, SocketAddr)>>> {
        loop {
            match &mut self.state {
                AcceptMultiState::Idle => {
                    self.state = AcceptMultiState::Accepting(Action::accept_multi(self.fd)?);
                }
                AcceptMultiState::Accepting(action) => {
                    if let Some(res) = action.get_mut().next() {
                        let socket = unsafe { Socket::from_raw_fd(res.result? as i32) };
                        let socket_addr = socket.peer_addr()?;
                        return Poll::Ready(Some(Ok((socket, socket_addr))));
                    }
                    let res = ready!(Pin::new(action).poll(cx));
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

struct Inner {
    accept: AcceptState,
    accept_unix: AcceptUnixState,
}

enum AcceptUnixState {
    Idle,
    Accepting(Action<driver::AcceptUnix>),
}

enum AcceptState {
    Idle,
    Accepting(Action<driver::Accept>),
}

pub(crate) struct AcceptMulti {
    fd: RawFd,
    state: AcceptMultiState,
}

enum AcceptMultiState {
    Idle,
    Accepting(Action<driver::AcceptMulti>),
    Done,
}
