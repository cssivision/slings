use std::io;
use std::os::unix::io::RawFd;
use std::ptr;

use io_uring::{opcode, types};

use crate::driver::Action;

pub struct Accept;

impl Action<Accept> {
    pub(crate) fn accept(fd: RawFd) -> io::Result<Action<Accept>> {
        let entry = opcode::Accept::new(types::Fd(fd), ptr::null_mut(), ptr::null_mut())
            .flags(libc::SOCK_CLOEXEC)
            .build();
        Action::submit(Accept, entry)
    }
}
