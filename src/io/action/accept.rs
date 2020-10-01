use std::future::Future;
use std::io::{self, Error, ErrorKind};
use std::mem;
use std::net::{SocketAddr, TcpStream};
use std::os::unix::io::{FromRawFd, RawFd};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use super::Action;
use crate::io::completion::Completion;

use io_uring::opcode::{self, types};
use nix::sys::socket::InetAddr;

pub struct AcceptAction {
    pub waker: Option<Waker>,
    pub ret: Option<io::Result<i32>>,
    pub sockaddr: Box<libc::sockaddr_storage>,
    pub socklen: u32,
}

pub struct Accept {
    action: Arc<Mutex<AcceptAction>>,
}

impl Accept {
    fn poll_accept(&self, cx: &mut Context) -> Poll<io::Result<(TcpStream, SocketAddr)>> {
        let mut action = self.action.lock().unwrap();
        if let Some(ret) = action.ret.take() {
            match ret {
                Ok(ret) => {
                    return Poll::Ready(Ok((
                        unsafe { TcpStream::from_raw_fd(ret) },
                        sockaddr_to_addr(&action.sockaddr, action.socklen as usize)?,
                    )));
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

fn sockaddr_to_addr(storage: &libc::sockaddr_storage, len: usize) -> io::Result<SocketAddr> {
    match storage.ss_family as libc::c_int {
        libc::AF_INET => {
            assert!(len as usize >= mem::size_of::<libc::sockaddr_in>());
            Ok(
                InetAddr::V4(unsafe { *(storage as *const _ as *const libc::sockaddr_in) })
                    .to_std(),
            )
        }
        libc::AF_INET6 => {
            assert!(len as usize >= mem::size_of::<libc::sockaddr_in6>());
            Ok(
                InetAddr::V6(unsafe { *(storage as *const _ as *const libc::sockaddr_in6) })
                    .to_std(),
            )
        }
        _ => Err(Error::new(ErrorKind::InvalidInput, "invalid argument")),
    }
}

pub fn accept(fd: RawFd) -> io::Result<Accept> {
    let sockaddr: libc::sockaddr_storage = unsafe { mem::zeroed() };
    let mut socklen = mem::size_of_val(&sockaddr) as libc::socklen_t;

    let mut sockaddr = Box::new(sockaddr);

    let entry = opcode::Accept::new(
        types::Fd(fd),
        &mut *sockaddr as *mut _ as *mut _,
        &mut socklen,
    )
    .build();

    let accept_action = Arc::new(Mutex::new(AcceptAction {
        sockaddr,
        socklen,
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
    type Output = io::Result<(TcpStream, SocketAddr)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_accept(cx)
    }
}
