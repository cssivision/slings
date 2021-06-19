use std::future::Future;
use std::sync::Arc;

use async_task::{Runnable, Task};
use concurrent_queue::ConcurrentQueue;
use scoped_tls::scoped_thread_local;

const MAX_TASKS_PER_TICK: usize = 64;

pub struct LocalExecutor {
    queue: Arc<ConcurrentQueue<Runnable>>,
}

scoped_thread_local!(static EXECUTOR: LocalExecutor);

pub fn spawn_local<T: 'static>(future: impl Future<Output = T> + 'static) -> Task<T> {
    if !EXECUTOR.is_set() {
        panic!("`spawn` called from outside of a `LocalExecutor`");
    }

    EXECUTOR.with(|local_executor| local_executor.spawn_local(future))
}

impl LocalExecutor {
    pub fn new() -> LocalExecutor {
        LocalExecutor {
            queue: Arc::new(ConcurrentQueue::unbounded()),
        }
    }

    pub fn tick(&self) -> bool {
        for _ in 0..MAX_TASKS_PER_TICK {
            match self.next_task() {
                Some(task) => {
                    task.run();
                }
                None => return false,
            }
        }
        true
    }

    fn next_task(&self) -> Option<Runnable> {
        if let Ok(task) = self.queue.pop() {
            Some(task)
        } else {
            None
        }
    }

    pub fn spawn_local<T: 'static>(&self, future: impl Future<Output = T> + 'static) -> Task<T> {
        let queue = self.queue.clone();
        let schedule = move |runnable| {
            let _ = queue.push(runnable);
        };

        let (runnable, task) = async_task::spawn_local(future, schedule);
        runnable.schedule();
        task
    }

    pub(crate) fn with<T>(&self, f: impl FnOnce() -> T) -> T {
        EXECUTOR.set(&self, f)
    }
}
