use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::os::unix::io::RawFd;
use std::rc::Rc;

use async_task::{Runnable, Task};
use scoped_tls::scoped_thread_local;

use crate::driver::notify;

const MAX_TASKS_PER_TICK: usize = 64;

pub struct LocalExecutor {
    queue: Rc<RefCell<VecDeque<Runnable>>>,
    event_fd: RawFd,
}

scoped_thread_local!(static EXECUTOR: LocalExecutor);

pub fn spawn_local<T: 'static>(future: impl Future<Output = T> + 'static) -> Task<T> {
    if !EXECUTOR.is_set() {
        panic!("`spawn_local` called from outside of a `LocalExecutor`");
    }

    EXECUTOR.with(|local_executor| local_executor.spawn_local(future))
}

impl LocalExecutor {
    pub fn new(event_fd: RawFd) -> LocalExecutor {
        LocalExecutor {
            queue: Rc::new(RefCell::new(VecDeque::with_capacity(64))),
            event_fd,
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
        self.queue.borrow_mut().pop_front()
    }

    pub fn spawn_local<T: 'static>(&self, future: impl Future<Output = T> + 'static) -> Task<T> {
        let queue = self.queue.clone();
        let event_fd = self.event_fd;
        let schedule = move |runnable| {
            let _ = queue.borrow_mut().push_back(runnable);
            notify(event_fd);
        };

        let (runnable, task) = unsafe { async_task::spawn_unchecked(future, schedule) };
        runnable.schedule();
        task
    }

    pub(crate) fn with<T>(&self, f: impl FnOnce() -> T) -> T {
        EXECUTOR.set(self, f)
    }
}
