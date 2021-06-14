use std::io;
use std::mem::{size_of, MaybeUninit};
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::Action;

pub(crate) struct Accept {
    pub addr: Box<MaybeUninit<libc::sockaddr_storage>>,
}

impl Action<Accept> {
    pub(crate) fn accept(fd: RawFd) -> io::Result<Action<Accept>> {
        let addr: MaybeUninit<libc::sockaddr_storage> = MaybeUninit::uninit();
        let mut accept = Accept {
            addr: Box::new(addr),
        };
        let mut length = size_of::<libc::sockaddr_storage>() as libc::socklen_t;
        let entry = opcode::Accept::new(
            types::Fd(fd),
            accept.addr.as_mut_ptr() as *mut _ as *mut _,
            &mut length,
        )
        .flags(libc::SOCK_CLOEXEC)
        .build();

        Action::submit(accept, entry)
    }
}
