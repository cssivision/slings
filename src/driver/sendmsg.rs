use std::io::{self, IoSliceMut};
use std::net::SocketAddr;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};
use socket2::SockAddr;

use crate::driver::{Action, Completable, CqeResult};

#[allow(dead_code)]
pub(crate) struct SendMsg {
    socket_addr: Box<SockAddr>,
    buf: Vec<u8>,
    io_slices: Vec<IoSliceMut<'static>>,
    msghdr: Box<libc::msghdr>,
}

impl Action<SendMsg> {
    pub(crate) fn sendmsg(
        fd: RawFd,
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
        };
        let entry = opcode::SendMsg::new(types::Fd(fd), send_msg.msghdr.as_mut() as *mut _).build();
        Action::submit(send_msg, entry)
    }
}

impl Completable for SendMsg {
    type Output = io::Result<usize>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        let n = cqe.result? as usize;
        Ok(n)
    }
}
