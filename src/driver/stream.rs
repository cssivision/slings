use std::io;
use std::os::unix::io::{AsRawFd, RawFd};
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::driver::buffers::ProvidedBuf;
use crate::driver::{self, Action};

use crate::driver::DEFAULT_BUFFER_SIZE;

pub struct Stream<T> {
    inner: Inner,
    io: T,
}

impl<T: AsRawFd> Stream<T> {
    pub fn new(io: T) -> Stream<T> {
        Stream {
            io,
            inner: Inner {
                read_pos: 0,
                rd: ProvidedBuf::default(),
                read: Read::Idle,
                write: Write::Idle,
            },
        }
    }

    pub fn get_ref(&self) -> &T {
        &self.io
    }

    pub fn poll_read(&mut self, cx: &mut Context, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let src = ready!(self.inner.poll_fill_buf(cx, self.io.as_raw_fd()))?;
        let n = buf.len().min(src.len());
        buf[..n].copy_from_slice(&src[..n]);
        self.inner.consume(n);
        Poll::Ready(Ok(n))
    }

    pub fn poll_fill_buf(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        self.inner.poll_fill_buf(cx, self.io.as_raw_fd())
    }

    pub fn consume(&mut self, amt: usize) {
        self.inner.consume(amt)
    }

    pub fn poll_write(&mut self, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.inner.poll_write(cx, buf, self.io.as_raw_fd())
    }
}

struct Inner {
    rd: ProvidedBuf,
    read_pos: usize,
    read: Read,
    write: Write,
}

enum Write {
    Idle,
    Writing(Action<driver::Write>),
}

enum Read {
    Idle,
    Reading(Action<driver::Read>),
}

impl Inner {
    fn poll_write(&mut self, cx: &mut Context, buf: &[u8], fd: RawFd) -> Poll<io::Result<usize>> {
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

    fn poll_fill_buf(&mut self, cx: &mut Context, fd: RawFd) -> Poll<io::Result<&[u8]>> {
        loop {
            match &mut self.read {
                Read::Idle => {
                    if !self.rd[self.read_pos..].is_empty() {
                        return Poll::Ready(Ok(&self.rd[self.read_pos..]));
                    }

                    self.read_pos = 0;
                    self.rd = ProvidedBuf::default();
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
