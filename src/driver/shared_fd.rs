use std::cell::RefCell;
use std::future::Future;
use std::os::unix::io::{FromRawFd, RawFd};
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Poll, Waker};

use futures_util::future::poll_fn;

use super::close::Close;
use crate::driver::Action;

#[derive(Clone)]
pub struct SharedFd {
    inner: Rc<Inner>,
}

struct Inner {
    fd: RawFd,
    state: RefCell<State>,
}

#[allow(dead_code)]
enum State {
    Init,
    Waiting(Option<Waker>),
    Closing(Action<Close>),
    Closed,
}

impl SharedFd {
    pub fn new(fd: RawFd) -> SharedFd {
        SharedFd {
            inner: Rc::new(Inner {
                fd,
                state: RefCell::new(State::Init),
            }),
        }
    }

    pub fn raw_fd(&self) -> RawFd {
        self.inner.fd
    }

    #[allow(dead_code)]
    pub async fn close(mut self) {
        if let Some(inner) = Rc::get_mut(&mut self.inner) {
            inner.submit_close_action();
        }
        self.inner.closed().await;
    }
}

impl Inner {
    fn submit_close_action(&mut self) {
        let state = self.state.get_mut();
        let waker = if let State::Waiting(waker) = state {
            waker.take()
        } else {
            None
        };
        *state = match Action::close(self.fd) {
            Ok(v) => State::Closing(v),
            Err(_) => {
                let _ = unsafe { std::fs::File::from_raw_fd(self.fd) };
                State::Closed
            }
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    pub async fn closed(&self) {
        poll_fn(|cx| {
            let mut state = self.state.borrow_mut();
            match &mut *state {
                State::Init => {
                    *state = State::Waiting(Some(cx.waker().clone()));
                    Poll::Pending
                }
                State::Waiting(Some(waker)) => {
                    if !waker.will_wake(cx.waker()) {
                        *waker = cx.waker().clone();
                    }
                    Poll::Pending
                }
                State::Waiting(None) => {
                    *state = State::Waiting(Some(cx.waker().clone()));
                    Poll::Pending
                }
                State::Closing(action) => {
                    let _ = ready!(Pin::new(action).poll(cx));
                    *state = State::Closed;
                    Poll::Ready(())
                }
                State::Closed => Poll::Ready(()),
            }
        })
        .await;
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        match self.state.get_mut() {
            State::Init | State::Waiting(..) => {
                self.submit_close_action();
            }
            _ => {}
        }
    }
}
