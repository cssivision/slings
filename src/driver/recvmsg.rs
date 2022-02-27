use std::future::Future;
use std::io::{self, IoSliceMut};
use std::net::SocketAddr;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use io_uring::{opcode, types};
use os_socketaddr::OsSocketAddr;

use crate::driver::Action;

#[allow(dead_code)]
pub struct RecvMsg {
    pub(crate) os_socket_addr: Box<OsSocketAddr>,
    io_slices: Vec<IoSliceMut<'static>>,
    buf: Vec<u8>,
    pub(crate) msghdr: Box<libc::msghdr>,
}

impl Action<RecvMsg> {
    pub fn recvmsg(fd: RawFd, len: usize) -> io::Result<Action<RecvMsg>> {
        let mut buf = Vec::with_capacity(len);
        let mut io_slices = vec![IoSliceMut::new(unsafe {
            std::slice::from_raw_parts_mut(buf.as_mut_ptr(), len)
        })];
        let mut os_socket_addr = Box::new(OsSocketAddr::new());
        let mut msghdr: Box<libc::msghdr> = Box::new(unsafe { std::mem::zeroed() });
        msghdr.msg_iov = io_slices.as_mut_ptr().cast();
        msghdr.msg_iovlen = io_slices.len() as _;
        msghdr.msg_name = os_socket_addr.as_mut_ptr() as *mut libc::c_void;
        msghdr.msg_namelen = os_socket_addr.capacity();
        let mut recv_msg = RecvMsg {
            os_socket_addr,
            buf,
            msghdr,
            io_slices,
        };
        let entry = opcode::RecvMsg::new(types::Fd(fd), recv_msg.msghdr.as_mut() as *mut _).build();
        Action::submit(recv_msg, entry)
    }

    pub fn poll_recv_from(
        &mut self,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<io::Result<(usize, SocketAddr)>> {
        let completion = ready!(Pin::new(&mut *self).poll(cx));
        let n = completion.result? as usize;
        let mut action = completion.action;
        unsafe { action.buf.set_len(n) };
        buf[..n].copy_from_slice(&action.buf[..n]);
        let socket_addr: Option<SocketAddr> = (*action.os_socket_addr).into();
        Poll::Ready(Ok((n, socket_addr.unwrap())))
    }
}
