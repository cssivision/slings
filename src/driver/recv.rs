use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::ptr;
use std::task::{Context, Poll};

use io_uring::cqueue::buffer_select;
use io_uring::squeue::Flags;
use io_uring::{opcode, types};

use crate::driver::Action;

pub struct Recv;

impl Action<Recv> {
    pub fn recv(fd: RawFd, len: u32) -> io::Result<Action<Recv>> {
        let entry = opcode::Recv::new(types::Fd(fd), ptr::null_mut(), len)
            .buf_group(0)
            .build()
            .flags(Flags::BUFFER_SELECT);
        Action::submit(Recv, entry)
    }

    pub fn poll_recv(&mut self, cx: &mut Context, rd: &mut [u8]) -> Poll<io::Result<usize>> {
        let completion = ready!(Pin::new(&mut *self).poll(cx));
        let n = completion.result?;
        let bid = buffer_select(completion.flags).expect("buffer_select unimplemented");
        let driver = self.driver.inner.borrow();

        let buf = unsafe {
            let mut provided_buf = driver.buffers.select(bid, self.driver.clone());
            provided_buf.set_len(n as usize);
            provided_buf
        };

        let n = buf.len().min(rd.len());
        rd[..n].copy_from_slice(&buf[..n]);
        Poll::Ready(Ok(n))
    }
}
