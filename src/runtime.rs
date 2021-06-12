use std::future::Future;
use std::io;

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

    pub fn block_on<F>(&mut self, future: F) -> F::Output
    where
        F: Future,
    {
        block_on(self.local_executor.run_until(future))
    }
}
