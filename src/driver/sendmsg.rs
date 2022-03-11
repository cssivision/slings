use std::future::Future;
use std::io::{self, IoSliceMut};
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use io_uring::{opcode, types};
use socket2::SockAddr;

use crate::driver::{Action, SharedFd};

#[allow(dead_code)]
pub(crate) struct SendMsg {
    fd: SharedFd,
    pub(crate) socket_addr: Box<SockAddr>,
    pub(crate) buf: Vec<u8>,
    io_slices: Vec<IoSliceMut<'static>>,
    pub(crate) msghdr: Box<libc::msghdr>,
}

impl Action<SendMsg> {
    pub(crate) fn sendmsg(
        fd: &SharedFd,
        buf: &[u8],
        socket_addr: SocketAddr,
    ) -> io::Result<Action<SendMsg>> {
        let len = buf.len();
        let mut buf = buf.to_vec();
        let mut io_slices = vec![IoSliceMut::new(unsafe {
            std::slice::from_raw_parts_mut(buf.as_mut_ptr(), len)
        })];
        let socket_addr = Box::new(SockAddr::from(socket_addr));
        let mut msghdr: Box<libc::msghdr> = Box::new(unsafe { std::mem::zeroed() });
        msghdr.msg_iov = io_slices.as_mut_ptr().cast();
        msghdr.msg_iovlen = io_slices.len() as _;
        msghdr.msg_name = socket_addr.as_ptr() as *mut libc::c_void;
        msghdr.msg_namelen = socket_addr.len();
        let mut send_msg = SendMsg {
            buf,
            msghdr,
            socket_addr,
            io_slices,
            fd: fd.clone(),
        };
        let entry =
            opcode::SendMsg::new(types::Fd(fd.raw_fd()), send_msg.msghdr.as_mut() as *mut _)
                .build();
        Action::submit(send_msg, entry)
    }

    pub(crate) fn poll_send_to(&mut self, cx: &mut Context) -> Poll<io::Result<usize>> {
        let complete = ready!(Pin::new(self).poll(cx));
        let n = complete.result? as usize;
        Poll::Ready(Ok(n))
    }
}
