use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::ptr;
use std::task::{Context, Poll};

use io_uring::cqueue::buffer_select;
use io_uring::squeue::Flags;
use io_uring::{opcode, types};

use crate::driver::buffers::{ProvidedBuf, GROUP_ID};
use crate::driver::Action;

pub struct Read;

impl Action<Read> {
    pub fn read(fd: RawFd, len: u32) -> io::Result<Action<Read>> {
        let entry = opcode::Read::new(types::Fd(fd), ptr::null_mut(), len)
            .buf_group(GROUP_ID)
            .build()
            .flags(Flags::BUFFER_SELECT);
        Action::submit(Read, entry)
    }

    pub fn poll_read(&mut self, cx: &mut Context) -> Poll<io::Result<ProvidedBuf>> {
        let completion = ready!(Pin::new(&mut *self).poll(cx));
        let buf = match buffer_select(completion.flags) {
            Some(bid) => {
                let mut driver = self.driver.inner.borrow_mut();
                Some(unsafe { driver.buffers.select(bid, self.driver.clone()) })
            }
            None => None,
        };
        let n = completion.result?;
        if let Some(mut buf) = buf {
            unsafe { buf.set_len(n as usize) };
            return Poll::Ready(Ok(buf));
        }
        Poll::Ready(Err(io::Error::new(
            io::ErrorKind::Other,
            "unexpect poll_read branch",
        )))
    }
}
