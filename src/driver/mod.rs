use std::any::Any;
use std::cell::RefCell;
use std::io;
use std::mem::{self, size_of, MaybeUninit};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::ops::{Deref, DerefMut};
use std::panic;
use std::rc::Rc;
use std::slice;
use std::task::Waker;

use io_uring::squeue::Entry;
use io_uring::{cqueue, IoUring};
use scoped_tls::scoped_thread_local;
use slab::Slab;

pub mod accept;
pub mod action;
pub mod connect;
pub mod packet;
pub mod read;
pub mod recv;
pub mod recvmsg;
pub mod send;
pub mod sendmsg;
pub mod stream;
pub mod timeout;
pub mod write;

pub use action::Action;
pub use packet::Packet;
pub use read::Read;
pub use recv::Recv;
pub use recvmsg::RecvMsg;
pub use send::Send;
pub use sendmsg::SendMsg;
pub use stream::Stream;
pub use timeout::Timeout;
pub use write::Write;

pub const DEFAULT_BUFFER_SIZE: usize = 4096;

scoped_thread_local!(static CURRENT: Driver);

pub struct Driver {
    pub inner: Rc<RefCell<Inner>>,
}

impl Clone for Driver {
    fn clone(&self) -> Self {
        Driver {
            inner: self.inner.clone(),
        }
    }
}

pub struct Inner {
    ring: IoUring,
    actions: Slab<State>,
    // buffers: Buffers,
}

impl Driver {
    pub fn new() -> io::Result<Driver> {
        let ring = IoUring::new(256)?;
        // check if IORING_FEAT_FAST_POLL is supported
        if !ring.params().is_feature_fast_poll() {
            panic!("IORING_FEAT_FAST_POLL not supported");
        }

        let driver = Driver {
            inner: Rc::new(RefCell::new(Inner {
                ring,
                actions: Slab::new(),
            })),
        };
        Ok(driver)
    }

    pub fn wait(&self) -> io::Result<()> {
        let inner = &mut *self.inner.borrow_mut();
        let ring = &mut inner.ring;

        if let Err(e) = ring.submit_and_wait(1) {
            if e.raw_os_error() == Some(libc::EBUSY) {
                return Ok(());
            }
            if e.kind() == io::ErrorKind::Interrupted {
                return Ok(());
            }
            return Err(e);
        }

        let mut cq = ring.completion();
        cq.sync();
        for cqe in cq {
            let key = cqe.user_data();
            if key == u64::MAX {
                continue;
            }
            let action = &mut inner.actions[key as usize];
            if action.complete(cqe) {
                inner.actions.remove(key as usize);
            }
        }

        Ok(())
    }

    pub fn with<T>(&self, f: impl FnOnce() -> T) -> T {
        CURRENT.set(self, f)
    }

    pub fn submit(&self, sqe: Entry) -> io::Result<usize> {
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        let key = inner.actions.insert(State::Submitted);

        let ring = &mut inner.ring;
        if ring.submission().is_full() {
            ring.submit()?;
            ring.submission().sync();
        }

        let sqe = sqe.user_data(key as u64);
        unsafe {
            ring.submission().push(&sqe).expect("push entry fail");
        }
        ring.submit()?;
        Ok(key)
    }
}

#[derive(Debug)]
pub enum State {
    /// The operation has been submitted to uring and is currently in-flight
    Submitted,
    /// The submitter is waiting for the completion of the operation
    Waiting(Waker),
    /// The operation has completed.
    Completed(cqueue::Entry),
    /// Ignored
    Ignored(Box<dyn Any>),
}

impl State {
    pub fn complete(&mut self, cqe: cqueue::Entry) -> bool {
        match mem::replace(self, State::Submitted) {
            State::Submitted => {
                *self = State::Completed(cqe);
                false
            }
            State::Waiting(waker) => {
                *self = State::Completed(cqe);
                waker.wake();
                false
            }
            State::Ignored(..) => true,
            State::Completed(..) => unreachable!("invalid operation state"),
        }
    }
}

unsafe fn to_socket_addr(storage: *const libc::sockaddr_storage) -> io::Result<SocketAddr> {
    match (*storage).ss_family as libc::c_int {
        libc::AF_INET => {
            // Safety: if the ss_family field is AF_INET then storage must be a sockaddr_in.
            let addr: &libc::sockaddr_in = &*(storage as *const libc::sockaddr_in);
            let ip = Ipv4Addr::from(addr.sin_addr.s_addr.to_ne_bytes());
            let port = u16::from_be(addr.sin_port);
            Ok(SocketAddr::V4(SocketAddrV4::new(ip, port)))
        }
        libc::AF_INET6 => {
            // Safety: if the ss_family field is AF_INET6 then storage must be a sockaddr_in6.
            let addr: &libc::sockaddr_in6 = &*(storage as *const libc::sockaddr_in6);
            let ip = Ipv6Addr::from(addr.sin6_addr.s6_addr);
            let port = u16::from_be(addr.sin6_port);
            Ok(SocketAddr::V6(SocketAddrV6::new(
                ip,
                port,
                addr.sin6_flowinfo,
                addr.sin6_scope_id,
            )))
        }
        _ => Err(io::ErrorKind::InvalidInput.into()),
    }
}

#[repr(C)]
union SockAddrIn {
    v4: libc::sockaddr_in,
    v6: libc::sockaddr_in6,
}

impl SockAddrIn {
    fn as_ptr(&self) -> *const libc::sockaddr {
        self as *const _ as *const libc::sockaddr
    }
}

fn socket_addr(addr: &SocketAddr) -> (SockAddrIn, libc::socklen_t) {
    match addr {
        SocketAddr::V4(ref addr) => {
            // `s_addr` is stored as BE on all machine and the array is in BE order.
            // So the native endian conversion method is used so that it's never swapped.
            let sin_addr = libc::in_addr {
                s_addr: u32::from_ne_bytes(addr.ip().octets()),
            };
            let sockaddr_in = libc::sockaddr_in {
                sin_family: libc::AF_INET as libc::sa_family_t,
                sin_port: addr.port().to_be(),
                sin_addr,
                sin_zero: [0; 8],
            };
            let sockaddr = SockAddrIn { v4: sockaddr_in };
            let socklen = size_of::<libc::sockaddr_in>() as libc::socklen_t;
            (sockaddr, socklen)
        }
        SocketAddr::V6(ref addr) => {
            let sockaddr_in6 = libc::sockaddr_in6 {
                sin6_family: libc::AF_INET6 as libc::sa_family_t,
                sin6_port: addr.port().to_be(),
                sin6_addr: libc::in6_addr {
                    s6_addr: addr.ip().octets(),
                },
                sin6_flowinfo: addr.flowinfo(),
                sin6_scope_id: addr.scope_id(),
            };
            let sockaddr = SockAddrIn { v6: sockaddr_in6 };
            let socklen = size_of::<libc::sockaddr_in6>() as libc::socklen_t;
            (sockaddr, socklen)
        }
    }
}

fn cmsghdr(msg_name: *mut libc::sockaddr_storage, bufs: &mut [MaybeUninitSlice]) -> libc::msghdr {
    let msg_namelen = size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    let mut msg: libc::msghdr = unsafe { mem::zeroed() };
    msg.msg_name = msg_name.cast();
    msg.msg_namelen = msg_namelen;
    msg.msg_iov = bufs.as_mut_ptr().cast();
    msg.msg_iovlen = bufs.len() as _;
    msg
}

#[repr(transparent)]
struct MaybeUninitSlice {
    vec: libc::iovec,
}

impl<'a> MaybeUninitSlice {
    pub(crate) fn new(buf: &mut [u8], len: usize) -> MaybeUninitSlice {
        MaybeUninitSlice {
            vec: libc::iovec {
                iov_base: buf.as_mut_ptr().cast(),
                iov_len: len,
            },
        }
    }

    pub(crate) fn as_slice(&self) -> &[MaybeUninit<u8>] {
        unsafe { slice::from_raw_parts(self.vec.iov_base.cast(), self.vec.iov_len) }
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [MaybeUninit<u8>] {
        unsafe { slice::from_raw_parts_mut(self.vec.iov_base.cast(), self.vec.iov_len) }
    }
}

impl Deref for MaybeUninitSlice {
    type Target = [MaybeUninit<u8>];

    fn deref(&self) -> &[MaybeUninit<u8>] {
        self.as_slice()
    }
}

impl DerefMut for MaybeUninitSlice {
    fn deref_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        self.as_mut_slice()
    }
}
