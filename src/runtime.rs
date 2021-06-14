use std::future::Future;
use std::io;
use std::task::Poll;

use futures_util::future::poll_fn;

use crate::blocking::block_on;
use crate::driver::Driver;
use crate::local_executor::LocalExecutor;

pub struct Runtime {
    local_executor: LocalExecutor,
    driver: Driver,
}

impl Runtime {
    pub fn new() -> io::Result<Runtime> {
        let local_executor = LocalExecutor::new();
        let driver = Driver::new()?;

        Ok(Runtime {
            local_executor,
            driver,
        })
    }

    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        pin_mut!(future);
        self.driver.with(|| {
            self.local_executor.with(|| {
                block_on(poll_fn(|cx| {
                    self.local_executor.register(cx.waker());

                    loop {
                        if let Poll::Ready(output) = future.as_mut().poll(cx) {
                            return Poll::Ready(output);
                        }

                        if self.local_executor.tick() {
                            // If `tick` returns `true`, we need to notify the local future again:
                            // there are still tasks remaining in the run queue.
                            cx.waker().wake_by_ref();
                            return Poll::Pending;
                        }

                        self.driver.wait().expect("driver wait error");
                    }
                }))
            })
        })
    }
}
