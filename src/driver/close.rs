use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::Action;

pub(crate) struct Close {
    fd: RawFd,
}

impl Action<Close> {
    pub(crate) fn close(fd: RawFd) -> io::Result<Action<Close>> {
        let close = Close { fd };
        let entry = opcode::Close::new(types::Fd(close.fd)).build();
        Action::try_submit(close, entry)
    }
}
