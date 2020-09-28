use std::future::Future;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct Accept {}

pub fn accept(fd: RawFd) -> Accept {
    Accept {}
}

impl Future for Accept {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unimplemented!();
    }
}
