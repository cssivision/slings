use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use io_uring::{opcode, types};

use crate::driver::Action;

#[allow(dead_code)]
pub(crate) struct Shutdown;

impl Action<Shutdown> {
    pub(crate) fn shutdown(fd: RawFd, how: libc::c_int) -> io::Result<Action<Shutdown>> {
        let shutdown = Shutdown;
        let entry = opcode::Shutdown::new(types::Fd(fd), how).build();
        Action::submit(shutdown, entry)
    }

    pub(crate) fn poll_shutdown(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        let complete = ready!(Pin::new(self).poll(cx));
        complete.result?;
        Poll::Ready(Ok(()))
    }
}
