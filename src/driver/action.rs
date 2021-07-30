use std::future::Future;
use std::io;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

use io_uring::squeue::Entry;

use crate::driver::{self, Driver, State};

pub struct Action<T> {
    pub driver: Driver,
    pub action: Option<T>,
    key: u64,
}

impl<T> Action<T> {
    pub fn submit(action: T, entry: Entry) -> io::Result<Action<T>> {
        driver::CURRENT.with(|driver| {
            let key = driver.submit(entry)?;
            Ok(Action {
                driver: driver.clone(),
                action: Some(action),
                key,
            })
        })
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
        let key = me.key as usize;
        let state = mem::replace(&mut inner.actions[key], State::Submitted);

        match state {
            State::Submitted => {
                inner.actions[key] = State::Waiting(cx.waker().clone());
                Poll::Pending
            }
            State::Waiting(waker) => {
                if !waker.will_wake(cx.waker()) {
                    inner.actions[key] = State::Waiting(cx.waker().clone());
                } else {
                    inner.actions[key] = State::Waiting(waker);
                }
                Poll::Pending
            }
            State::Completed(cqe) => {
                inner.actions.remove(key);
                let result = if cqe.result() >= 0 {
                    Ok(cqe.result())
                } else {
                    Err(io::Error::from_raw_os_error(-cqe.result()))
                };
                let flags = cqe.flags();
                let action = me.action.take().expect("action can not be None");
                Poll::Ready(Completion {
                    action,
                    result,
                    flags,
                })
            }
        }
    }
}

pub struct Completion<T> {
    pub(crate) action: T,
    pub(crate) result: io::Result<i32>,
    pub(crate) flags: u32,
}
