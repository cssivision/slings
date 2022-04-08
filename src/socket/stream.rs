use std::io;
use std::net;
use std::pin::Pin;
use std::task::{Context, Poll};

use super::Socket;
use crate::driver::{self, Action, SharedFd};

const DEFAULT_BUFFER_SIZE: usize = 4096;

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
                read: Read::Idle,
                write: Write::Idle,
                shutdown: Shutdown::Idle,
            },
        }
    }

    pub(crate) fn get_ref(&self) -> &Socket {
        &self.io
    }

    pub(crate) fn poll_read(
        &mut self,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let src = ready!(self.inner.poll_fill_buf(cx, &self.io.fd))?;
        let n = buf.len().min(src.len());
        buf[..n].copy_from_slice(&src[..n]);
        self.inner.consume(n);
        Poll::Ready(Ok(n))
    }

    pub(crate) fn poll_fill_buf(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        self.inner.poll_fill_buf(cx, &self.io.fd)
    }

    pub(crate) fn consume(&mut self, amt: usize) {
        self.inner.consume(amt)
    }

    pub(crate) fn poll_write(&mut self, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.inner.poll_write(cx, buf, &self.io.fd)
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
        self.inner.poll_shutdown(cx, &self.io.fd, how)
    }
}

struct Inner {
    rd: Vec<u8>,
    read_pos: usize,
    read: Read,
    write: Write,
    shutdown: Shutdown,
}

enum Write {
    Idle,
    Writing(Action<driver::Write>),
}

enum Read {
    Idle,
    Reading(Action<driver::Read>),
}

enum Shutdown {
    Idle,
    Shutdowning(Action<driver::Shutdown>),
}

impl Inner {
    fn poll_shutdown(
        &mut self,
        cx: &mut Context,
        fd: &SharedFd,
        how: libc::c_int,
    ) -> Poll<io::Result<()>> {
        loop {
            match &mut self.shutdown {
                Shutdown::Idle => {
                    let action = Action::shutdown(fd, how)?;
                    self.shutdown = Shutdown::Shutdowning(action);
                }
                Shutdown::Shutdowning(action) => {
                    let n = ready!(Pin::new(action).poll_shutdown(cx))?;
                    self.shutdown = Shutdown::Idle;
                    return Poll::Ready(Ok(n));
                }
            }
        }
    }

    fn poll_write(
        &mut self,
        cx: &mut Context,
        buf: &[u8],
        fd: &SharedFd,
    ) -> Poll<io::Result<usize>> {
        loop {
            match &mut self.write {
                Write::Idle => {
                    let action = Action::write(fd, buf)?;
                    self.write = Write::Writing(action);
                }
                Write::Writing(action) => {
                    let n = ready!(Pin::new(action).poll_write(cx))?;
                    self.write = Write::Idle;
                    return Poll::Ready(Ok(n));
                }
            }
        }
    }

    fn poll_fill_buf(&mut self, cx: &mut Context, fd: &SharedFd) -> Poll<io::Result<&[u8]>> {
        loop {
            match &mut self.read {
                Read::Idle => {
                    if !self.rd[self.read_pos..].is_empty() {
                        return Poll::Ready(Ok(&self.rd[self.read_pos..]));
                    }

                    self.read_pos = 0;
                    self.rd = vec![];
                    let action = Action::read(fd, DEFAULT_BUFFER_SIZE as u32)?;
                    self.read = Read::Reading(action);
                }
                Read::Reading(action) => {
                    self.rd = ready!(Pin::new(action).poll_read(cx))?;
                    self.read = Read::Idle;
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
