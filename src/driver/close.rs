use std::io;
use std::os::unix::io::RawFd;

use io_uring::{opcode, types};

use crate::driver::Action;

pub struct Close {
    fd: RawFd,
}

impl Action<Close> {
    pub fn close(fd: RawFd) -> io::Result<Action<Close>> {
        let close = Close { fd };
        let entry = opcode::Close::new(types::Fd(close.fd)).build();
        Action::submit(close, entry)
    }
}
