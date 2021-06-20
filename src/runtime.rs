use std::future::Future;
use std::io;
use std::task::{Context, Poll};

use futures_util::future::poll_fn;

use crate::driver::Driver;
use crate::local_executor::LocalExecutor;
use crate::waker_fn::waker_fn;

pub struct Runtime {
    local_executor: LocalExecutor,
    driver: Driver,
}

impl Runtime {
    pub fn new() -> io::Result<Runtime> {
        Ok(Runtime {
            local_executor: LocalExecutor::new(),
            driver: Driver::new()?,
        })
    }

    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        pin_mut!(future);
        self.driver.with(|| {
            self.local_executor.with(|| {
                block_on(poll_fn(|cx| loop {
                    if let Poll::Ready(output) = future.as_mut().poll(cx) {
                        return Poll::Ready(output);
                    }
                    if self.local_executor.tick() {
                        return Poll::Pending;
                    }
                    self.driver.wait().expect("driver wait error");
                }))
            })
        })
    }
}

/// Runs a future to completion on the current thread.
fn block_on<T>(future: impl Future<Output = T>) -> T {
    pin_mut!(future);
    let waker = waker_fn(|| {});
    let cx = &mut Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => continue,
        }
    }
}
