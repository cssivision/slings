use std::future::Future;
use std::io;
use std::net;
use std::os::unix::io::{AsRawFd, RawFd};
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use socket2::SockAddr;

use super::Socket;
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
                read_pos: 0,
                rd: vec![],
                read: ReadState::Idle,
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
    rd: Vec<u8>,
    read_pos: usize,
    read: ReadState,
    write: WriteState,
    shutdown: ShutdownState,
    connect: ConnectState,
}

enum ConnectState {
    Idle,
    Connecting(Op<driver::Connect>),
    Done,
}

enum WriteState {
    Idle,
    Writing(Op<driver::Write>),
}

enum ReadState {
    Idle,
    Reading(Op<driver::Read>),
}

enum ShutdownState {
    Idle,
    Shutdowning(Op<driver::Shutdown>),
    Done,
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
                    self.connect = ConnectState::Done;
                }
                ConnectState::Done => {
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
                    self.shutdown = ShutdownState::Done;
                }
                ShutdownState::Done => {
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
        loop {
            match &mut self.read {
                ReadState::Idle => {
                    if !self.rd[self.read_pos..].is_empty() {
                        return Poll::Ready(Ok(&self.rd[self.read_pos..]));
                    }
                    self.read_pos = 0;
                    self.rd = vec![];
                    self.read = ReadState::Reading(Op::read(fd, DEFAULT_BUFFER_SIZE)?);
                }
                ReadState::Reading(op) => {
                    let cqe = ready!(Pin::new(&mut *op).poll(cx));
                    self.rd = op.get_buf(cqe)?;
                    self.read = ReadState::Idle;
                    self.read_pos = 0;
                    if self.rd.is_empty() {
                        return Poll::Ready(Ok(&self.rd[..]));
                    }
                }
            }
        }
    }

    fn consume(&mut self, amt: usize) {
        self.read_pos += amt;
    }
}
