use std::future::Future;
use std::io;
use std::mem::MaybeUninit;
use std::net::SocketAddr;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use io_uring::{opcode, types};

use crate::driver::Action;
use crate::driver::{cmsghdr, to_socket_addr, MaybeUninitSlice};

pub struct RecvMsg {
    storage: Box<MaybeUninit<libc::sockaddr_storage>>,
    buf: Vec<u8>,
}

impl Action<RecvMsg> {
    pub fn recvmsg(fd: RawFd, len: usize) -> io::Result<Action<RecvMsg>> {
        let mut storage = Box::new(MaybeUninit::<libc::sockaddr_storage>::zeroed());
        let mut buf = Vec::with_capacity(len);
        let mut iovec = [MaybeUninitSlice::new(&mut buf, len)];
        let mut msghdr = cmsghdr(storage.as_mut_ptr() as *mut _, &mut iovec);
        let entry = opcode::RecvMsg::new(types::Fd(fd), &mut msghdr as *mut _).build();
        Action::submit(RecvMsg { storage, buf }, entry)
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
