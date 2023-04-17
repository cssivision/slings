use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use io_uring::{opcode, types};

use crate::driver::{Action, SharedFd};

#[allow(dead_code)]
pub(crate) struct Shutdown {
    fd: SharedFd,
}

impl Action<Shutdown> {
    pub(crate) fn shutdown(fd: &SharedFd, how: libc::c_int) -> io::Result<Action<Shutdown>> {
        let shutdown = Shutdown { fd: fd.clone() };
        let entry = opcode::Shutdown::new(types::Fd(fd.raw_fd()), how).build();
        Action::submit(shutdown, entry)
    }

    pub(crate) fn poll_shutdown(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        let complete = ready!(Pin::new(self).poll(cx));
        complete.result?;
        Poll::Ready(Ok(()))
    }
}
