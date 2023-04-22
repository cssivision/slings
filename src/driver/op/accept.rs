use std::io;
use std::mem;
use std::net::SocketAddr;
use std::os::unix::io::RawFd;

use io_uring::squeue::Entry;
use io_uring::{opcode, types};
use socket2::SockAddr;

use crate::driver::{Action, Completable, CqeResult};
use crate::socket::{socketaddr, Socket};

pub struct Accept {
    pub(crate) socketaddr: Box<(libc::sockaddr_storage, libc::socklen_t)>,
}

impl Action<Accept> {
    pub(crate) fn accept(fd: RawFd) -> io::Result<Action<Accept>> {
        let (socketaddr, entry) = accept(fd);
        Action::submit(Accept { socketaddr }, entry)
    }
}

impl Completable for Accept {
    type Output = io::Result<(Socket, Option<SocketAddr>)>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        let fd = cqe.result? as i32;
        let socket = Socket { fd };
        let (_, addr) = unsafe {
            SockAddr::try_init(move |addr_storage, len| {
                *addr_storage = self.socketaddr.0.to_owned();
                *len = self.socketaddr.1;
                Ok(())
            })?
        };
        Ok((socket, addr.as_socket()))
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

pub struct AcceptUnix {
    pub(crate) socketaddr: Box<(libc::sockaddr_storage, libc::socklen_t)>,
}

impl Action<AcceptUnix> {
    pub(crate) fn accept_unix(fd: RawFd) -> io::Result<Action<AcceptUnix>> {
        let (socketaddr, entry) = accept(fd);
        Action::submit(AcceptUnix { socketaddr }, entry)
    }
}

impl Completable for AcceptUnix {
    type Output = io::Result<(Socket, socketaddr::SocketAddr)>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        let fd = cqe.result? as i32;
        let socket = Socket { fd };
        let mut storage = self.socketaddr.0.to_owned();
        let socklen = self.socketaddr.1;
        let storage: *mut libc::sockaddr_storage = &mut storage as *mut _;
        let sockaddr: libc::sockaddr_un = unsafe { *storage.cast() };
        Ok((
            socket,
            socketaddr::SocketAddr::from_parts(sockaddr, socklen),
        ))
    }
}
