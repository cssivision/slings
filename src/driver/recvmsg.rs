use std::future::Future;
use std::io::{self, IoSliceMut};
use std::net::SocketAddr;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use io_uring::{opcode, types};
use socket2::SockAddr;

use crate::driver::Action;

#[allow(dead_code)]
pub(crate) struct RecvMsg {
    pub(crate) socket_addr: Box<SockAddr>,
    io_slices: Vec<IoSliceMut<'static>>,
    buf: Vec<u8>,
    pub(crate) msghdr: Box<libc::msghdr>,
}

impl Action<RecvMsg> {
    pub(crate) fn recvmsg(fd: RawFd, len: usize) -> io::Result<Action<RecvMsg>> {
        let mut buf = Vec::with_capacity(len);
        let mut io_slices = vec![IoSliceMut::new(unsafe {
            std::slice::from_raw_parts_mut(buf.as_mut_ptr(), len)
        })];
        let socket_addr = Box::new(unsafe { SockAddr::init(|_, _| Ok(()))?.1 });
        let mut msghdr: Box<libc::msghdr> = Box::new(unsafe { std::mem::zeroed() });
        msghdr.msg_iov = io_slices.as_mut_ptr().cast();
        msghdr.msg_iovlen = io_slices.len() as _;
        msghdr.msg_name = socket_addr.as_ptr() as *mut libc::c_void;
        msghdr.msg_namelen = socket_addr.len();
        let mut recv_msg = RecvMsg {
            socket_addr,
            buf,
            msghdr,
            io_slices,
        };
        let entry = opcode::RecvMsg::new(types::Fd(fd), recv_msg.msghdr.as_mut() as *mut _).build();
        Action::submit(recv_msg, entry)
    }

    pub(crate) fn poll_recv_from(
        &mut self,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<io::Result<(usize, SocketAddr)>> {
        let completion = ready!(Pin::new(&mut *self).poll(cx));
        let n = completion.result? as usize;
        let mut action = completion.action;
        unsafe { action.buf.set_len(n) };
        buf[..n].copy_from_slice(&action.buf[..n]);
        let socket_addr = action
            .socket_addr
            .as_socket()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid argument"))?;
        Poll::Ready(Ok((n, socket_addr)))
    }
}
