use std::future::Future;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use crate::completion;

#[derive(Debug)]
pub struct AcceptAction {
    pub waker: Option<Waker>,
    pub fd: Option<i32>,
    pub sockaddr: String,
    pub socklen: u32,
}

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
