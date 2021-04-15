use std::io;
use std::net;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::io::action;

use bytes::BytesMut;
use futures_util::io::AsyncRead;

#[derive(Debug)]
pub struct TcpStream {
    buf: BytesMut,
    inner: net::TcpStream,
}

impl TcpStream {
    pub fn from_std(stream: net::TcpStream) -> TcpStream {
        TcpStream {
            inner: stream,
            buf: BytesMut::new(),
        }
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        if self.buf.len() >= buf.len() {
            let data = self.buf.split_to(buf.len());
            buf.clone_from_slice(&data);
            return Poll::Ready(Ok(buf.len()));
        }

        self.buf.reserve(buf.len());

        unimplemented!();
    }
}
