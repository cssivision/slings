use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::ptr;
use std::task::{Context, Poll};

use io_uring::cqueue::buffer_select;
use io_uring::squeue::Flags;
use io_uring::{opcode, types};

use crate::driver::buffers::ProvidedBuf;
use crate::driver::Action;

pub struct Read;

impl Action<Read> {
    pub fn read(fd: RawFd, len: u32) -> io::Result<Action<Read>> {
        let entry = opcode::Read::new(types::Fd(fd), ptr::null_mut(), len)
            .buf_group(0)
            .build()
            .flags(Flags::BUFFER_SELECT);
        Action::submit(Read, entry)
    }

    pub fn poll_read(&mut self, cx: &mut Context) -> Poll<io::Result<ProvidedBuf>> {
        let completion = ready!(Pin::new(&mut *self).poll(cx));
        let n = completion.result?;
        let bid = buffer_select(completion.flags).expect("buffer_select unimplemented");
        let mut driver = self.driver.inner.borrow_mut();

        let buf = unsafe {
            let mut provided_buf = driver.buffers.select(bid, self.driver.clone());
            provided_buf.set_len(n as usize);
            provided_buf
        };
        Poll::Ready(Ok(buf))
    }
}
