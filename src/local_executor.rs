use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;

use async_task::{Runnable, Task};

const MAX_TASKS_PER_TICK: usize = 64;

thread_local! {
    static GLOBAL_QUEUE: RefCell<VecDeque<Runnable>> = RefCell::new(VecDeque::with_capacity(64));
}

pub fn tick() -> bool {
    for _ in 0..MAX_TASKS_PER_TICK {
        match next_task() {
            Some(task) => {
                task.run();
            }
            None => return false,
        }
    }
    true
}

fn next_task() -> Option<Runnable> {
    GLOBAL_QUEUE.with(|queue| queue.borrow_mut().pop_front())
}

pub fn spawn_local<T: 'static>(future: impl Future<Output = T> + 'static) -> Task<T> {
    let schedule = move |runnable| {
        let _ = GLOBAL_QUEUE.with(|queue| queue.borrow_mut().push_back(runnable));
    };

    let (runnable, task) = unsafe { async_task::spawn_unchecked(future, schedule) };
    runnable.schedule();
    task
}
