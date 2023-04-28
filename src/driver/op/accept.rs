use std::io;
use std::mem;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::{Completable, CqeResult, Op};
use crate::socket::Socket;

pub(crate) struct Accept {
    pub(crate) socketaddr: Box<(libc::sockaddr_storage, libc::socklen_t)>,
}

impl Op<Accept> {
    pub(crate) fn accept(fd: RawFd) -> io::Result<Op<Accept>> {
        let mut socketaddr = Box::new((
            unsafe { mem::zeroed() },
            mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t,
        ));
        let entry = opcode::Accept::new(
            types::Fd(fd),
            &mut socketaddr.0 as *mut _ as *mut _,
            &mut socketaddr.1,
        )
        .flags(libc::SOCK_CLOEXEC)
        .build();
        Op::submit(Accept { socketaddr }, entry)
    }
}

impl Completable for Accept {
    type Output = io::Result<(Socket, Box<(libc::sockaddr_storage, libc::socklen_t)>)>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        let fd = cqe.result? as i32;
        Ok((Socket::from(fd), self.socketaddr))
    }
}
