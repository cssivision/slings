use std::future::Future;
use std::io;
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
    pub fd: Option<i32>,
    pub sockaddr: Option<libc::sockaddr>,
}

pub struct Accept {
    action: Arc<Mutex<AcceptAction>>,
}

impl Accept {
    fn poll_accept(&self, cx: &mut Context) -> Poll<io::Result<(RawFd, SocketAddr)>> {
        let mut action = self.action.lock().unwrap();
        if let Some(fd) = action.fd {
            if let Some(sockaddr) = action.sockaddr.take() {
                return Poll::Ready(Ok((fd, InetAddr::V4(to_sockaddr_in(sockaddr)).to_std())));
            }
        }

        if action.waker.is_none() {
            action.waker = Some(cx.waker().clone());
        }

        Poll::Pending
    }
}

fn to_sockaddr_in(sockaddr: libc::sockaddr) -> libc::sockaddr_in {
    let mut sin_zero: [u8; 8] = Default::default();
    sin_zero.copy_from_slice(
        &sockaddr.sa_data[6..14]
            .to_vec()
            .iter()
            .map(|v| *v as u8)
            .collect::<Vec<u8>>(),
    );

    libc::sockaddr_in {
        sin_family: sockaddr.sa_family,
        sin_port: (sockaddr.sa_data[0] as u16) << 8 | sockaddr.sa_data[1] as u16,
        sin_addr: libc::in_addr {
            s_addr: (sockaddr.sa_data[2] as u32) << 24
                | (sockaddr.sa_data[3] as u32) << 16
                | (sockaddr.sa_data[4] as u32) << 8
                | (sockaddr.sa_data[5] as u32),
        },
        sin_zero: sin_zero,
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
        fd: None,
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
