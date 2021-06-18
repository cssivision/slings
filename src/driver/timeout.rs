use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use io_uring::{opcode, types};

use crate::driver::Action;

pub(crate) struct Timeout {
    spec: types::Timespec,
}

impl Action<Timeout> {
    pub(crate) fn timeout(sec: u64, nsec: u32) -> io::Result<Action<Timeout>> {
        let timeout = Timeout {
            spec: types::Timespec::new().sec(sec).nsec(nsec),
        };
        let entry = opcode::Timeout::new(&timeout.spec as *const _).build();
        Action::submit(timeout, entry)
    }

    pub(crate) fn poll_timeout(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let completion = ready!(Pin::new(&mut *self).poll(cx));
        let result = completion.result;

        match result {
            Err(err) if err.raw_os_error() == Some(libc::ETIME) => {}
            Err(err) if err.raw_os_error() == Some(libc::ECANCELED) => {}
            _ => {}
        }
        Poll::Ready(Ok(()))
    }
}
