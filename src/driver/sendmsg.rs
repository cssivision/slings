use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use io_uring::{opcode, types};

use crate::driver::Action;
use crate::driver::{cmsghdr, socket_addr};

pub struct SendMsg {
    _buf: Vec<u8>,
    _msghdr: libc::msghdr,
}

impl Action<SendMsg> {
    pub fn sendmsg(fd: RawFd, buf: &[u8], addr: &SocketAddr) -> io::Result<Action<SendMsg>> {
        let mut buf = buf.to_vec();
        let (addr, _) = socket_addr(addr);
        let msghdr = cmsghdr(addr.as_ptr() as *mut _, &mut buf);
        let entry = opcode::SendMsg::new(types::Fd(fd), &msghdr).build();
        Action::submit(
            SendMsg {
                _buf: buf,
                _msghdr: msghdr,
            },
            entry,
        )
    }

    pub(crate) fn poll_send_to(&mut self, cx: &mut Context) -> Poll<io::Result<usize>> {
        let complete = ready!(Pin::new(self).poll(cx));
        let n = complete.result? as usize;
        Poll::Ready(Ok(n))
    }
}
