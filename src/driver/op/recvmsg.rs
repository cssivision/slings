use std::io::{self, IoSliceMut};
use std::net::SocketAddr;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};
use socket2::SockAddr;

use crate::driver::{Completable, CqeResult, Op};

#[allow(dead_code)]
pub(crate) struct RecvMsg {
    socket_addr: Box<SockAddr>,
    io_slices: Vec<IoSliceMut<'static>>,
    buf: Vec<u8>,
    msghdr: Box<libc::msghdr>,
}

impl Op<RecvMsg> {
    pub(crate) fn recvmsg(fd: RawFd, len: usize) -> io::Result<Op<RecvMsg>> {
        let mut buf = Vec::with_capacity(len);
        let mut io_slices = vec![IoSliceMut::new(unsafe {
            std::slice::from_raw_parts_mut(buf.as_mut_ptr(), len)
        })];
        let socket_addr = Box::new(unsafe { SockAddr::try_init(|_, _| Ok(()))?.1 });
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
        Op::submit(recv_msg, entry)
    }
}

impl Completable for RecvMsg {
    type Output = io::Result<(Vec<u8>, SocketAddr)>;

    fn complete(mut self, cqe: CqeResult) -> Self::Output {
        let n = cqe.result? as usize;
        unsafe { self.buf.set_len(n) };
        let socket_addr = self
            .socket_addr
            .as_socket()
            .ok_or(io::ErrorKind::InvalidInput)?;
        Ok((self.buf, socket_addr))
    }
}
