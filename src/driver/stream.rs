use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::driver::buffers::ProvidedBuf;
use crate::driver::{self, Action};

const DEFAULT_BUF_SIZE: usize = 4096;

pub(crate) struct Stream {
    fd: RawFd,
    inner: Inner,
}

impl Stream {
    pub(crate) fn new(fd: RawFd) -> Stream {
        Stream {
            fd,
            inner: Inner {
                read_pos: 0,
                read_buf: ProvidedBuf::default(),
                read: Read::Idle,
                write_pos: 0,
                write_buf: Vec::with_capacity(DEFAULT_BUF_SIZE),
                write: Write::Idle,
            },
        }
    }

    pub(crate) fn poll_read(
        &mut self,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let src = ready!(self.inner.poll_fill_buf(cx, self.fd))?;
        let n = buf.len().min(src.len());
        buf[..n].copy_from_slice(&src[..n]);
        self.inner.consume(n);
        Poll::Ready(Ok(n))
    }

    pub(crate) fn poll_fill_buf(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        self.inner.poll_fill_buf(cx, self.fd)
    }

    pub(crate) fn consume(&mut self, amt: usize) {
        self.inner.consume(amt)
    }
}

struct Inner {
    read_buf: ProvidedBuf,
    read_pos: usize,
    read: Read,
    write_pos: usize,
    write_buf: Vec<u8>,
    write: Write,
}

enum Write {
    Idle,
    Reading(Action<driver::Write>),
}

enum Read {
    Idle,
    Reading(Action<driver::Read>),
}

impl Inner {
    fn poll_fill_buf(&mut self, cx: &mut Context, fd: RawFd) -> Poll<io::Result<&[u8]>> {
        loop {
            match &mut self.read {
                Read::Idle => {
                    if !self.read_buf[self.read_pos..].is_empty() {
                        return Poll::Ready(Ok(&self.read_buf[self.read_pos..]));
                    }

                    self.read_pos = 0;
                    self.read_buf = ProvidedBuf::default();
                    let action = Action::read(fd, DEFAULT_BUF_SIZE as u32)?;
                    self.read = Read::Reading(action);
                }
                Read::Reading(action) => {
                    self.read_buf = ready!(Pin::new(action).poll_read(cx))?;
                    if self.read_buf.is_empty() {
                        return Poll::Ready(Ok(&self.read_buf[self.read_pos..]));
                    }
                    self.read = Read::Idle;
                }
            }
        }
    }

    fn consume(&mut self, amt: usize) {
        self.read_pos += amt;
    }
}
