use std::io::{self, IoSliceMut};
use std::net::SocketAddr;
use std::os::unix::io::RawFd;
use std::ptr;

use io_uring::{opcode, squeue, types};
use socket2::SockAddr;

use crate::driver::{Buf, Completable, CqeResult, Op, BUF_BGID};

#[allow(dead_code)]
pub(crate) struct RecvMsg {
    socket_addr: Box<SockAddr>,
    io_slices: Vec<IoSliceMut<'static>>,
    msghdr: Box<libc::msghdr>,
}

impl Op<RecvMsg> {
    pub(crate) fn recvmsg(fd: RawFd, len: usize) -> io::Result<Op<RecvMsg>> {
        let mut io_slices = vec![IoSliceMut::new(unsafe {
            std::slice::from_raw_parts_mut(ptr::null_mut(), len)
        })];
        let socket_addr = Box::new(unsafe { SockAddr::try_init(|_, _| Ok(()))?.1 });
        let mut msghdr: Box<libc::msghdr> = Box::new(unsafe { std::mem::zeroed() });
        msghdr.msg_iov = io_slices.as_mut_ptr().cast();
        msghdr.msg_iovlen = io_slices.len() as _;
        msghdr.msg_name = socket_addr.as_ptr() as *mut libc::c_void;
        msghdr.msg_namelen = socket_addr.len();
        let mut recv_msg = RecvMsg {
            socket_addr,
            msghdr,
            io_slices,
        };
        let entry = opcode::RecvMsg::new(types::Fd(fd), recv_msg.msghdr.as_mut() as *mut _)
            .buf_group(BUF_BGID)
            .build()
            .flags(squeue::Flags::BUFFER_SELECT);
        Op::submit(recv_msg, entry)
    }
}

impl Completable for RecvMsg {
    type Output = io::Result<(Buf, SocketAddr)>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        let _ = cqe.result?;
        let buf = match cqe.buf {
            Some(buf) => Ok(buf),
            None => Err(io::Error::new(io::ErrorKind::Other, "buf not found")),
        }?;
        let socket_addr = self
            .socket_addr
            .as_socket()
            .ok_or(io::ErrorKind::InvalidInput)?;
        Ok((buf, socket_addr))
    }
}
