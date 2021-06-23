use std::future::Future;
use std::io;
use std::task::{Context, Poll};

use crate::driver::Driver;
use crate::local_executor::LocalExecutor;
use crate::waker_fn::waker_fn;

/// Runs a closure when dropped.
struct CallOnDrop<F: Fn()>(F);

impl<F: Fn()> Drop for CallOnDrop<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

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

        let waker = waker_fn(|| {});
        let cx = &mut Context::from_waker(&waker);

        self.driver.with(|| {
            self.local_executor.with(|| loop {
                if let Poll::Ready(output) = future.as_mut().poll(cx) {
                    return output;
                }

                if self.local_executor.tick() {
                    continue;
                }

                self.driver.wait().expect("driver wait error");
            })
        })
    }
}
