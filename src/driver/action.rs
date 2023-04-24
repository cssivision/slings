use std::future::Future;
use std::io;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use io_uring::cqueue;
use io_uring::squeue::Entry;

use crate::driver::{self, Driver, State};

pub(crate) trait Completable {
    type Output;
    /// `complete` will be called for cqe's do not have the `more` flag set
    fn complete(self, cqe: CqeResult) -> Self::Output;
    /// Update will be called for cqe's which have the `more` flag set.
    /// The Op should update any internal state as required.
    fn update(&mut self, _cqe: CqeResult) {}
}

pub(crate) struct Action<T: 'static> {
    pub driver: Driver,
    pub action: Option<T>,
    pub key: usize,
}

impl<T> Action<T> {
    pub(crate) fn get_mut(&mut self) -> &mut T {
        self.action.as_mut().unwrap()
    }

    pub(crate) fn submit(action: T, entry: Entry) -> io::Result<Action<T>> {
        driver::CURRENT.with(|driver| driver.submit(action, entry))
    }

    pub(crate) fn insert_waker(&self, waker: Waker) {
        let mut inner = self.driver.inner.borrow_mut();
        let state = inner.actions.get_mut(self.key).expect("invalid state key");
        *state = State::Waiting(waker);
    }

    fn poll2(&mut self, cx: &mut Context) -> Poll<T::Output>
    where
        T: 'static + Completable,
    {
        let mut inner = self.driver.inner.borrow_mut();
        let state = inner.actions.get_mut(self.key).expect("invalid state key");

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
            State::Completed(cqe) => {
                inner.actions.remove(self.key);
                Poll::Ready(self.action.take().unwrap().complete(cqe))
            }
            State::CompletionList(list) => {
                let data = self.action.as_mut().unwrap();
                let mut status = None;
                let mut updated = false;
                for cqe in list.into_iter() {
                    if cqueue::more(cqe.flags) {
                        updated = true;
                        data.update(cqe);
                    } else {
                        status = Some(cqe);
                        break;
                    }
                }
                if updated {
                    // because we update internal state, wake and rerun the task.
                    cx.waker().wake_by_ref();
                }
                match status {
                    None => {
                        *state = State::Waiting(cx.waker().clone());
                    }
                    Some(cqe) => {
                        *state = State::Completed(cqe);
                    }
                }
                Poll::Pending
            }
            State::Ignored(..) => unreachable!(),
        }
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
            State::CompletionList(list) => {
                let more = if !list.is_empty() {
                    io_uring::cqueue::more(list.last().unwrap().flags)
                } else {
                    false
                };
                if more {
                    *state = State::Ignored(Box::new(self.action.take()));
                } else {
                    inner.actions.remove(self.key);
                }
            }
            State::Ignored(..) => unreachable!(),
        }
    }
}

impl<T> Future for Action<T>
where
    T: Unpin + 'static + Completable,
{
    type Output = T::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.as_mut().poll2(cx)
    }
}

#[allow(dead_code)]
pub(crate) struct CqeResult {
    pub(crate) result: io::Result<u32>,
    pub(crate) flags: u32,
}

impl From<cqueue::Entry> for CqeResult {
    fn from(cqe: cqueue::Entry) -> Self {
        let res = cqe.result();
        let flags = cqe.flags();
        let result = if res >= 0 {
            Ok(res as u32)
        } else {
            Err(io::Error::from_raw_os_error(-res))
        };
        CqeResult { result, flags }
    }
}
