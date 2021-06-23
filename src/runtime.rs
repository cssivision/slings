use std::cell::Cell;
use std::future::Future;
use std::io;
use std::task::{Context, Poll};

use crate::driver::{notify, Driver};
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
        let driver = Driver::new()?;
        let event_fd = driver.event_fd();
        Ok(Runtime {
            local_executor: LocalExecutor::new(event_fd),
            driver,
        })
    }

    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        pin_mut!(future);
        thread_local! {
            static IO_BLOCKED: Cell<bool> = Cell::new(false);
        }

        let event_fd = self.driver.event_fd();
        let waker = waker_fn(move || {
            if IO_BLOCKED.with(Cell::get) {
                notify(event_fd);
            }
        });
        let cx = &mut Context::from_waker(&waker);

        self.driver.with(|| {
            self.local_executor.with(|| loop {
                if let Poll::Ready(output) = future.as_mut().poll(cx) {
                    return output;
                }

                if self.local_executor.tick() {
                    continue;
                }

                IO_BLOCKED.with(|io| io.set(true));
                let _guard = CallOnDrop(|| {
                    IO_BLOCKED.with(|io| io.set(false));
                });
                self.driver.wait().expect("driver wait error");
            })
        })
    }
}
