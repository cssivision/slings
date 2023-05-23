use std::future::Future;
use std::io;
use std::net;
use std::os::unix::io::{AsRawFd, RawFd};
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use socket2::SockAddr;

use super::Socket;
use crate::buffer::Buf;
use crate::driver::{self, Op};

const DEFAULT_BUFFER_SIZE: u32 = 4096;

pub(crate) struct Stream {
    inner: Inner,
    io: Socket,
}

impl Stream {
    pub(crate) fn new(io: Socket) -> Stream {
        Stream {
            io,
            inner: Inner {
                read: Read {
                    pos: 0,
                    buf: None,
                    state: ReadState::Idle,
                },
                write: WriteState::Idle,
                shutdown: ShutdownState::Idle,
                connect: ConnectState::Idle,
            },
        }
    }

    pub(crate) fn get_ref(&self) -> &Socket {
        &self.io
    }

    pub(crate) fn poll_connect(
        &mut self,
        cx: &mut Context,
        addr: &SockAddr,
    ) -> Poll<io::Result<()>> {
        self.inner.poll_connect(cx, self.io.as_raw_fd(), addr)
    }

    pub(crate) fn poll_read(
        &mut self,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let src = ready!(self.inner.poll_fill_buf(cx, self.io.as_raw_fd()))?;
        let n = buf.len().min(src.len());
        buf[..n].copy_from_slice(&src[..n]);
        self.inner.consume(n);
        Poll::Ready(Ok(n))
    }

    pub(crate) fn poll_fill_buf(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        self.inner.poll_fill_buf(cx, self.io.as_raw_fd())
    }

    pub(crate) fn consume(&mut self, amt: usize) {
        self.inner.consume(amt)
    }

    pub(crate) fn poll_write(&mut self, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.inner.poll_write(cx, buf, self.io.as_raw_fd())
    }

    pub(crate) fn poll_shutdown(
        &mut self,
        cx: &mut Context,
        how: net::Shutdown,
    ) -> Poll<io::Result<()>> {
        let how = match how {
            net::Shutdown::Write => libc::SHUT_WR,
            net::Shutdown::Read => libc::SHUT_RD,
            net::Shutdown::Both => libc::SHUT_RDWR,
        };
        self.inner.poll_shutdown(cx, self.io.as_raw_fd(), how)
    }
}

struct Inner {
    read: Read,
    write: WriteState,
    shutdown: ShutdownState,
    connect: ConnectState,
}

enum ConnectState {
    Idle,
    Connecting(Op<driver::Connect>),
}

enum WriteState {
    Idle,
    Writing(Op<driver::Write>),
}

enum ReadState {
    Idle,
    Reading(Op<driver::Read>),
}

struct Read {
    buf: Option<Buf>,
    pos: usize,
    state: ReadState,
}

impl Read {
    fn poll_fill_buf(&mut self, cx: &mut Context, fd: RawFd) -> Poll<io::Result<&[u8]>> {
        loop {
            match &mut self.state {
                ReadState::Idle => {
                    if self.buf.is_some() && !self.buf.as_ref().unwrap()[self.pos..].is_empty() {
                        return Poll::Ready(Ok(&self.buf.as_ref().unwrap()[self.pos..]));
                    }
                    self.pos = 0;
                    self.buf = None;
                    self.state = ReadState::Reading(Op::read(fd, DEFAULT_BUFFER_SIZE)?);
                }
                ReadState::Reading(op) => {
                    let buf = ready!(Pin::new(&mut *op).poll(cx))?;
                    self.state = ReadState::Idle;
                    self.pos = 0;
                    self.buf = Some(buf);
                    // if length of buf is zero, means EOF.
                    if self.buf.as_ref().unwrap().is_empty() {
                        return Poll::Ready(Ok(&self.buf.as_ref().unwrap()[..]));
                    }
                }
            }
        }
    }

    fn consume(&mut self, amt: usize) {
        self.pos += amt;
    }
}

enum ShutdownState {
    Idle,
    Shutdowning(Op<driver::Shutdown>),
}

impl Inner {
    fn poll_connect(
        &mut self,
        cx: &mut Context,
        fd: RawFd,
        addr: &SockAddr,
    ) -> Poll<io::Result<()>> {
        loop {
            match &mut self.connect {
                ConnectState::Idle => {
                    self.connect = ConnectState::Connecting(Op::connect(fd, addr.clone())?);
                }
                ConnectState::Connecting(op) => {
                    ready!(Pin::new(op).poll(cx))?;
                    self.connect = ConnectState::Idle;
                    return Poll::Ready(Ok(()));
                }
            }
        }
    }

    fn poll_shutdown(
        &mut self,
        cx: &mut Context,
        fd: RawFd,
        how: libc::c_int,
    ) -> Poll<io::Result<()>> {
        loop {
            match &mut self.shutdown {
                ShutdownState::Idle => {
                    self.shutdown = ShutdownState::Shutdowning(Op::shutdown(fd, how)?);
                }
                ShutdownState::Shutdowning(op) => {
                    ready!(Pin::new(op).poll(cx))?;
                    self.shutdown = ShutdownState::Idle;
                    return Poll::Ready(Ok(()));
                }
            }
        }
    }

    fn poll_write(&mut self, cx: &mut Context, buf: &[u8], fd: RawFd) -> Poll<io::Result<usize>> {
        loop {
            match &mut self.write {
                WriteState::Idle => {
                    self.write = WriteState::Writing(Op::write(fd, buf)?);
                }
                WriteState::Writing(op) => {
                    let n = ready!(Pin::new(&mut *op).poll(cx))?;
                    self.write = WriteState::Idle;
                    return Poll::Ready(Ok(n));
                }
            }
        }
    }

    fn poll_fill_buf(&mut self, cx: &mut Context, fd: RawFd) -> Poll<io::Result<&[u8]>> {
        self.read.poll_fill_buf(cx, fd)
    }

    fn consume(&mut self, amt: usize) {
        self.read.consume(amt);
    }
}
