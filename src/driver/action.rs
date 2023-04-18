use std::future::Future;
use std::io;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use io_uring::squeue::Entry;

use crate::driver::{self, Driver, State};

pub(crate) struct Action<T: 'static> {
    pub driver: Driver,
    pub action: Option<T>,
    pub key: usize,
}

impl<T> Action<T> {
    pub(crate) fn submit(action: T, entry: Entry) -> io::Result<Action<T>> {
        driver::CURRENT.with(|driver| driver.submit(action, entry))
    }

    pub(crate) fn insert_waker(&self, waker: Waker) {
        let mut inner = self.driver.inner.borrow_mut();
        let state = inner.actions.get_mut(self.key).expect("invalid state key");
        *state = State::Waiting(waker);
    }
}

impl<T> Drop for Action<T> {
    fn drop(&mut self) {
        let mut inner = self.driver.inner.borrow_mut();
        let state = match inner.actions.get_mut(self.key) {
            Some(s) => s,
            None => return,
        };

        match state {
            State::Submitted | State::Waiting(_) => {
                *state = State::Ignored(Box::new(self.action.take()));
            }
            State::Completed(..) => {
                inner.actions.remove(self.key);
            }
            State::Ignored(..) => unreachable!(),
        }
    }
}

impl<T> Future for Action<T>
where
    T: Unpin,
{
    type Output = Completion<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let me = &mut *self;
        let mut inner = me.driver.inner.borrow_mut();
        let state = inner.actions.get_mut(me.key).expect("invalid state key");

        match mem::replace(state, State::Submitted) {
            State::Submitted => {
                *state = State::Waiting(cx.waker().clone());
                Poll::Pending
            }
            State::Waiting(waker) => {
                if !waker.will_wake(cx.waker()) {
                    *state = State::Waiting(cx.waker().clone());
                } else {
                    *state = State::Waiting(waker);
                }
                Poll::Pending
            }
            State::Ignored(..) => unreachable!(),
            State::Completed(cqe) => {
                inner.actions.remove(me.key);
                me.key = usize::MAX;
                let result = if cqe.result() >= 0 {
                    Ok(cqe.result())
                } else {
                    Err(io::Error::from_raw_os_error(-cqe.result()))
                };
                let flags = cqe.flags();
                Poll::Ready(Completion {
                    action: me.action.take().expect("action can not be None"),
                    result,
                    flags,
                })
            }
        }
    }
}

#[allow(dead_code)]
pub(crate) struct Completion<T> {
    pub(crate) action: T,
    pub(crate) result: io::Result<i32>,
    pub(crate) flags: u32,
}
