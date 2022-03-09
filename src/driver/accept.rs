use std::io;

use io_uring::{opcode, types};

use crate::driver::shared_fd::SharedFd;
use crate::driver::Action;

#[allow(dead_code)]
pub struct Accept {
    fd: SharedFd,
    pub(crate) socketaddr: Box<(libc::sockaddr_storage, libc::socklen_t)>,
}

impl Action<Accept> {
    pub(crate) fn accept(fd: &SharedFd) -> io::Result<Action<Accept>> {
        let mut socketaddr = Box::new((
            unsafe { std::mem::zeroed() },
            std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t,
        ));
        let entry = opcode::Accept::new(
            types::Fd(fd.raw_fd()),
            &mut socketaddr.0 as *mut _ as *mut _,
            &mut socketaddr.1,
        )
        .flags(libc::SOCK_CLOEXEC)
        .build();
        Action::submit(
            Accept {
                fd: fd.clone(),
                socketaddr,
            },
            entry,
        )
    }
}
