use std::future::Future;
use std::io;
use std::ops::Sub;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

pub use delay::{delay_for, delay_until, Delay};

use crate::driver::{self, Action};

pub mod delay;
pub mod timeout;

enum State {
    Idle,
    Waiting(Action<driver::Timeout>),
}

pub struct Timeout {
    deadline: Instant,
    state: State,
}

impl Timeout {
    pub fn new(deadline: Instant) -> Timeout {
        Timeout {
            deadline,
            state: State::Idle,
        }
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    pub fn is_elapsed(&self) -> bool {
        self.deadline < Instant::now()
    }

    pub fn reset(&mut self, when: Instant) {
        self.state = State::Idle;
        self.deadline = when;
    }
}

impl Future for Timeout {
    type Output = io::Result<Instant>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<Instant>> {
        loop {
            match &mut self.state {
                State::Idle => {
                    let duration = self.deadline.sub(Instant::now());
                    let action = Action::timeout(duration.as_secs(), duration.subsec_nanos())?;
                    self.state = State::Waiting(action);
                }
                State::Waiting(action) => {
                    ready!(Pin::new(action).poll_timeout(cx))?;
                    return Poll::Ready(Ok(self.deadline));
                }
            }
        }
    }
}
