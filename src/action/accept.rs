use std::future::Future;
use std::io;
use std::mem::transmute;
use std::net::SocketAddr;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::ptr;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use super::Action;
use crate::completion::Completion;

use io_uring::opcode::{self, types};
use nix::sys::socket::InetAddr;

pub struct AcceptAction {
    pub waker: Option<Waker>,
    pub ret: Option<io::Result<i32>>,
    pub sockaddr: Option<libc::sockaddr>,
}

pub struct Accept {
    action: Arc<Mutex<AcceptAction>>,
}

impl Accept {
    fn poll_accept(&self, cx: &mut Context) -> Poll<io::Result<(RawFd, SocketAddr)>> {
        let mut action = self.action.lock().unwrap();
        if let Some(ret) = action.ret.take() {
            match ret {
                Ok(ret) => {
                    if let Some(sockaddr) = action.sockaddr.take() {
                        return Poll::Ready(Ok((
                            ret,
                            InetAddr::V4(*to_sockaddr_in(sockaddr)).to_std(),
                        )));
                    }
                }
                Err(e) => return Poll::Ready(Err(e)),
            }
        }

        if action.waker.is_none() {
            action.waker = Some(cx.waker().clone());
        }

        Poll::Pending
    }
}

fn to_sockaddr_in(sockaddr: libc::sockaddr) -> Box<libc::sockaddr_in> {
    let sockaddr = Box::into_raw(Box::new(sockaddr));

    unsafe {
        let sockaddr_in = transmute::<*mut libc::sockaddr, *mut libc::sockaddr_in>(sockaddr);
        Box::from_raw(sockaddr_in)
    }
}

pub fn accept(fd: RawFd) -> io::Result<Accept> {
    let mut sockaddr = libc::sockaddr {
        sa_family: 0,
        sa_data: [0i8; 14],
    };

    let entry = opcode::Accept::new(types::Fd(fd), &mut sockaddr, ptr::null_mut()).build();

    let accept_action = Arc::new(Mutex::new(AcceptAction {
        sockaddr: Some(sockaddr),
        waker: None,
        ret: None,
    }));

    let action = Action::Accept {
        inner: accept_action.clone(),
    };

    let action = Arc::new(action);
    let key = Completion::get().insert(action.clone());

    let entry = entry.user_data(key as _);
    Completion::get().submit(entry)?;

    Ok(Accept {
        action: accept_action,
    })
}

impl Future for Accept {
    type Output = io::Result<(RawFd, SocketAddr)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_accept(cx)
    }
}
