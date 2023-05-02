use std::task::{Context, Poll, Waker};
use std::time::Instant;

use crate::driver;

pub mod delay;
pub mod interval;
pub mod timeout;

pub use delay::{delay_for, delay_until, Delay};
pub use interval::{interval, interval_at, Interval};
pub use timeout::{timeout, timeout_at, Timeout};

pub(crate) struct Timer {
    deadline: Instant,
    key: usize,
    waker: Option<Waker>,
}

impl Timer {
    pub fn new(deadline: Instant) -> Timer {
        Timer {
            deadline,
            key: 0,
            waker: None,
        }
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    pub fn is_elapsed(&self) -> bool {
        self.deadline < Instant::now()
    }

    fn insert_timer(&self, when: Instant, waker: &Waker) -> usize {
        driver::CURRENT.with(|driver| driver.insert_timer(when, waker))
    }

    fn remove_timer(&self) {
        driver::CURRENT.with(|driver| driver.remove_timer(self.deadline, self.key))
    }

    pub fn reset(&mut self, when: Instant) {
        if let Some(waker) = self.waker.as_ref() {
            self.remove_timer();
            self.key = self.insert_timer(when, waker);
        }
        self.deadline = when;
    }

    pub fn poll_timeout(&mut self, cx: &mut Context) -> Poll<Instant> {
        if self.deadline <= Instant::now() {
            if self.key > 0 {
                self.remove_timer();
            }
            Poll::Ready(self.deadline)
        } else {
            match self.waker {
                None => {
                    self.key = self.insert_timer(self.deadline, cx.waker());
                    self.waker = Some(cx.waker().clone());
                }
                Some(ref w) if !w.will_wake(cx.waker()) => {
                    self.remove_timer();
                    self.key = self.insert_timer(self.deadline, cx.waker());
                    self.waker = Some(cx.waker().clone());
                }
                _ => {}
            }
            Poll::Pending
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(_) = self.waker.take() {
            self.remove_timer();
        }
    }
}
