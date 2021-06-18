use std::io;
use std::os::unix::io::{AsRawFd, RawFd};
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::driver::buffers::ProvidedBuf;
use crate::driver::{self, Action};

const DEFAULT_BUF_SIZE: usize = 4096;

pub(crate) struct Stream<T> {
    inner: Inner,
    io: T,
}

impl<T> Stream<T> {
    pub fn get_ref(&self) -> &T {
        &self.io
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.io
    }
}

impl<T: AsRawFd> Stream<T> {
    pub(crate) fn new(io: T) -> Stream<T> {
        Stream {
            io,
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

    pub(crate) fn poll_flush(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        self.inner.poll_flush(cx, self.io.as_raw_fd())
    }
}

struct Inner {
    read_buf: ProvidedBuf,
    read_pos: usize,
    read: Read,
    write_buf: Vec<u8>,
    write_pos: usize,
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
        let n = self.write_buf.capacity() - self.write_buf.len();
        if n == 0 {
            ready!(self.poll_flush(cx, fd))?;
        }

        let size = n.min(buf.len());
        self.write_buf.extend_from_slice(&buf[..size]);
        assert!(
            self.write_buf.capacity() == DEFAULT_BUF_SIZE,
            "write buf capacity should not grow"
        );
        Poll::Ready(Ok(n))
    }

    fn poll_flush(&mut self, cx: &mut Context, fd: RawFd) -> Poll<io::Result<()>> {
        loop {
            match &mut self.write {
                Write::Idle => {
                    if self.write_buf[self.write_pos..].is_empty() {
                        self.write_buf.clear();
                        self.write_pos = 0;
                        return Poll::Ready(Ok(()));
                    }

                    let action = Action::write(fd, &self.write_buf[self.write_pos..])?;
                    self.write = Write::Writing(action);
                }
                Write::Writing(action) => {
                    let n = ready!(Pin::new(action).poll_write(cx))?;
                    self.write_pos += n;
                    self.write = Write::Idle;
                }
            }
        }
    }

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
