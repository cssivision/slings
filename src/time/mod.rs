use std::future::Future;
use std::io;
use std::ops::Sub;
use std::pin::Pin;
use std::task::{ready, Context, Poll, Waker};
use std::time::Instant;

use crate::driver::{self, Op};

pub mod delay;
pub mod interval;
pub mod timeout;

pub use delay::{delay_for, delay_until, Delay};
pub use interval::{interval, interval_at, Interval};
pub use timeout::{timeout, timeout_at, Timeout};

enum TimeoutState {
    Idle,
    Waiting(Op<driver::Timeout>),
}

pub struct Timer {
    deadline: Instant,
    state: TimeoutState,
    waker: Option<Waker>,
}

impl Timer {
    pub fn new(deadline: Instant) -> Timer {
        Timer {
            deadline,
            state: TimeoutState::Idle,
            waker: None,
        }
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    pub fn is_elapsed(&self) -> bool {
        self.deadline < Instant::now()
    }

    pub fn reset(&mut self, when: Instant) {
        self.state = TimeoutState::Idle;
        self.deadline = when;
        if let Some(waker) = self.waker.as_ref() {
            let duration = self.deadline.sub(Instant::now());
            let op = Op::timeout(duration.as_secs(), duration.subsec_nanos())
                .expect("fail to submit timeout sqe");
            op.reset(waker.clone());
            self.state = TimeoutState::Waiting(op);
        }
    }

    fn poll_timeout(&mut self, cx: &mut Context) -> Poll<io::Result<Instant>> {
        if self.deadline <= Instant::now() {
            return Poll::Ready(Ok(self.deadline));
        }

        loop {
            match &mut self.state {
                TimeoutState::Idle => {
                    let duration = self.deadline.sub(Instant::now());
                    let op = Op::timeout(duration.as_secs(), duration.subsec_nanos())?;
                    self.state = TimeoutState::Waiting(op);
                }
                TimeoutState::Waiting(op) => {
                    match &self.waker {
                        Some(waker) if !waker.will_wake(cx.waker()) => {
                            self.waker = Some(cx.waker().clone());
                        }
                        None => {
                            self.waker = Some(cx.waker().clone());
                        }
                        _ => {}
                    }
                    ready!(Pin::new(op).poll(cx))?;
                    self.state = TimeoutState::Idle;
                    self.waker = None;
                    return Poll::Ready(Ok(self.deadline));
                }
            }
        }
    }
}
