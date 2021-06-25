use std::future::Future;
use std::io;
use std::mem::{self, MaybeUninit};
use std::net::SocketAddr;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use io_uring::{opcode, types};

use crate::driver::Action;
use crate::driver::{cmsghdr, to_socket_addr};

pub struct RecvMsg {
    storage: Box<MaybeUninit<libc::sockaddr_storage>>,
    msghdr: libc::msghdr,
    _iovec: libc::iovec,
    buf: Vec<u8>,
}

impl Action<RecvMsg> {
    pub fn recvmsg(fd: RawFd, len: usize) -> io::Result<Action<RecvMsg>> {
        let mut storage = Box::new(mem::MaybeUninit::<libc::sockaddr_storage>::zeroed());
        let buf = Vec::with_capacity(len);
        let iovec = libc::iovec {
            iov_base: &buf as *const _ as _,
            iov_len: len,
        };
        let bufs = unsafe { std::slice::from_raw_parts_mut(iovec.iov_base.cast(), iovec.iov_len) };
        let msghdr = cmsghdr(storage.as_mut_ptr() as *mut _, bufs.as_mut_ptr());
        let mut recv_msg = RecvMsg {
            storage,
            msghdr,
            buf,
            _iovec: iovec,
        };
        let entry = opcode::RecvMsg::new(types::Fd(fd), &mut recv_msg.msghdr as *mut _).build();
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
        let addr = unsafe { to_socket_addr(action.storage.as_mut_ptr() as *const _)? };
        Poll::Ready(Ok((n, addr)))
    }
}
