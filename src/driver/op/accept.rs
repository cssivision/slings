use std::io;
use std::mem;
use std::os::unix::io::RawFd;

use io_uring::squeue::Entry;
use io_uring::{opcode, types};

use crate::driver::{Action, Completable, CqeResult};
use crate::socket::Socket;

pub(crate) struct Accept {
    pub(crate) socketaddr: Box<(libc::sockaddr_storage, libc::socklen_t)>,
}

impl Action<Accept> {
    pub(crate) fn accept(fd: RawFd) -> io::Result<Action<Accept>> {
        let (socketaddr, entry) = accept(fd);
        Action::submit(Accept { socketaddr }, entry)
    }
}

impl Completable for Accept {
    type Output = io::Result<(Socket, Box<(libc::sockaddr_storage, libc::socklen_t)>)>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        let fd = cqe.result? as i32;
        let socket = Socket { fd };
        Ok((socket, self.socketaddr))
    }
}

fn accept(fd: RawFd) -> (Box<(libc::sockaddr_storage, libc::socklen_t)>, Entry) {
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
    (socketaddr, entry)
}
