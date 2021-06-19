use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use super::Timeout;

pub struct Delay {
    timeout: Timeout,
}

pub fn delay_until(deadline: Instant) -> Delay {
    Delay {
        timeout: Timeout::new(deadline),
    }
}

pub fn delay_for(duration: Duration) -> Delay {
    delay_until(Instant::now() + duration)
}

impl Delay {
    pub fn deadline(&self) -> Instant {
        self.timeout.deadline()
    }

    pub fn is_elapsed(&self) -> bool {
        self.timeout.is_elapsed()
    }

    pub fn reset(&mut self, deadline: Instant) {
        self.timeout.reset(deadline);
    }
}

impl Future for Delay {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        match Pin::new(&mut self.timeout).poll(cx) {
            Poll::Ready(_) => Poll::Ready(()),
            Poll::Pending => Poll::Pending,
        }
    }
}
