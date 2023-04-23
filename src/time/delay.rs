use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use super::Timer;

pub struct Delay {
    inner: Timer,
}

pub fn delay_until(deadline: Instant) -> Delay {
    Delay {
        inner: Timer::new(deadline),
    }
}

pub fn delay_for(duration: Duration) -> Delay {
    delay_until(Instant::now() + duration)
}

impl Delay {
    pub fn deadline(&self) -> Instant {
        self.inner.deadline()
    }

    pub fn is_elapsed(&self) -> bool {
        self.inner.is_elapsed()
    }

    pub fn reset(&mut self, deadline: Instant) {
        self.inner.reset(deadline);
    }
}

impl Future for Delay {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.inner.poll_timeout(cx) {
            Poll::Ready(v) => match v {
                Ok(_) => Poll::Ready(()),
                Err(e) => panic!("timer err: {}", e),
            },
            Poll::Pending => Poll::Pending,
        }
    }
}
