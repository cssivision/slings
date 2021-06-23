use std::cell::RefCell;
use std::io;
use std::mem;
use std::os::unix::io::RawFd;
use std::panic;
use std::rc::Rc;
use std::task::Waker;

use io_uring::opcode::{PollAdd, ProvideBuffers};
use io_uring::squeue::Entry;
use io_uring::{cqueue, types, IoUring};
use scoped_tls::scoped_thread_local;
use slab::Slab;

pub(crate) mod accept;
pub(crate) mod action;
pub(crate) mod buffers;
pub(crate) mod connect;
pub(crate) mod read;
pub(crate) mod stream;
pub(crate) mod timeout;
pub(crate) mod write;

pub use action::Action;
use buffers::Buffers;
pub use read::Read;
pub use stream::Stream;
pub use timeout::Timeout;
pub use write::Write;

pub const DEFAULT_BUFFER_SIZE: usize = 2048;
const DEFAULT_BUFFER_NUM: usize = 1024;

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
    buffers: Buffers,
    event_fd: RawFd,
}

pub fn notify(event_fd: RawFd) {
    if !CURRENT.is_set() {
        panic!("`notify` called from outside of a `driver`");
    }

    let buf: [u8; 8] = 1u64.to_ne_bytes();
    let _ = syscall!(write(
        event_fd,
        &buf[0] as *const u8 as *const libc::c_void,
        buf.len()
    ));
}

impl Driver {
    pub fn get_event_fd(&self) -> RawFd {
        self.inner.borrow().event_fd
    }

    pub fn new() -> io::Result<Driver> {
        let mut ring = IoUring::new(256)?;
        let event_fd = syscall!(eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK))?;

        // check if IORING_FEAT_FAST_POLL is supported
        if !ring.params().is_feature_fast_poll() {
            panic!("IORING_FEAT_FAST_POLL not supported");
        }

        // check if buffer selection is supported
        let mut probe = io_uring::Probe::new();
        ring.submitter().register_probe(&mut probe).unwrap();
        if !probe.is_supported(ProvideBuffers::CODE) {
            panic!("buffer selection not supported");
        }
        let buffers = Buffers::new(DEFAULT_BUFFER_NUM, DEFAULT_BUFFER_SIZE);
        provide_buffers(&mut ring, &buffers)?;

        let driver = Driver {
            inner: Rc::new(RefCell::new(Inner {
                ring,
                actions: Slab::new(),
                buffers,
                event_fd,
            })),
        };
        Ok(driver)
    }

    pub fn wait(&self) -> io::Result<()> {
        let inner = &mut *self.inner.borrow_mut();
        let ring = &mut inner.ring;

        poll_add(ring, inner.event_fd)?;

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
            action.complete(cqe);
        }

        // try read notified
        let mut buf = [0u8; 8];
        let _ = syscall!(read(
            inner.event_fd,
            &mut buf[0] as *mut u8 as *mut libc::c_void,
            buf.len()
        ));
        Ok(())
    }

    pub fn with<T>(&self, f: impl FnOnce() -> T) -> T {
        CURRENT.set(self, f)
    }

    pub fn submit(&self, sqe: Entry) -> io::Result<u64> {
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        let key = inner.actions.insert(State::Submitted) as u64;

        let ring = &mut inner.ring;
        if ring.submission().is_full() {
            ring.submit()?;
            ring.submission().sync();
        }

        let sqe = sqe.user_data(key);
        unsafe {
            ring.submission().push(&sqe).expect("push entry fail");
        }
        ring.submit()?;
        Ok(key)
    }
}

fn poll_add(ring: &mut IoUring, event_fd: RawFd) -> io::Result<()> {
    let entry = PollAdd::new(types::Fd(event_fd), libc::EPOLLIN as _)
        .build()
        .user_data(u64::MAX);
    if ring.submission().is_full() {
        ring.submit()?;
        ring.submission().sync();
    }
    unsafe {
        ring.submission().push(&entry).expect("submit entry fail");
    }
    ring.submit()?;
    Ok(())
}

fn provide_buffers(ring: &mut IoUring, buffers: &Buffers) -> io::Result<()> {
    let entry = ProvideBuffers::new(buffers.mem, buffers.size as i32, buffers.num as u16, 0, 0)
        .build()
        .user_data(0);
    unsafe {
        ring.submission().push(&entry).expect("push entry fail");
    }
    ring.submit_and_wait(1)?;
    for cqe in ring.completion() {
        let ret = cqe.result();
        if cqe.user_data() != 0 {
            panic!("provide_buffers user_data error");
        }
        if ret < 0 {
            panic!("provide_buffers submit error, ret: {}", ret);
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum State {
    /// The operation has been submitted to uring and is currently in-flight
    Submitted,
    /// The submitter is waiting for the completion of the operation
    Waiting(Waker),
    /// The operation has completed.
    Completed(cqueue::Entry),
}

impl State {
    pub fn complete(&mut self, cqe: cqueue::Entry) {
        match mem::replace(self, State::Submitted) {
            State::Submitted => {
                *self = State::Completed(cqe);
            }
            State::Waiting(waker) => {
                *self = State::Completed(cqe);
                waker.wake();
            }
            State::Completed(_) => unreachable!("invalid operation state"),
        };
    }
}
