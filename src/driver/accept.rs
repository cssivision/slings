use std::io;
use std::mem;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::Action;

#[allow(dead_code)]
pub struct Accept {
    pub(crate) socketaddr: Box<(libc::sockaddr_storage, libc::socklen_t)>,
}

impl Action<Accept> {
    pub(crate) fn accept(fd: RawFd) -> io::Result<Action<Accept>> {
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
        Action::submit(Accept { socketaddr }, entry)
    }
}
