use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use async_task::{Runnable, Task};
use concurrent_queue::ConcurrentQueue;
use pin_project_lite::pin_project;
use scoped_tls::scoped_thread_local;

const MAX_TASKS_PER_TICK: usize = 64;

pub struct LocalExecutor {
    queue: ConcurrentQueue<Runnable>,
    waker: RefCell<Option<Waker>>,
}

scoped_thread_local!(static CURRENT: LocalExecutor);

pub fn spawn<T: 'static>(future: impl Future<Output = T> + 'static) -> Task<T> {
    if !CURRENT.is_set() {
        panic!("`spawn` called from outside of a `LocalExecutor`");
    }

    CURRENT.with(|local_executor| local_executor.spawn(future))
}

impl LocalExecutor {
    pub fn new() -> LocalExecutor {
        LocalExecutor {
            queue: ConcurrentQueue::unbounded(),
            waker: RefCell::new(None),
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

    fn wake(&self) {
        let waker = self.waker.borrow_mut().take();
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    fn register(&self, waker: &Waker) {
        *self.waker.borrow_mut() = Some(waker.clone());
    }

    pub fn spawn<T: 'static>(&self, future: impl Future<Output = T> + 'static) -> Task<T> {
        self.with(|| {
            let schedule = |runnable| {
                CURRENT.with(|local_executor| {
                    let _ = local_executor.queue.push(runnable);
                    local_executor.wake();
                });
            };

            let (runnable, task) = async_task::spawn_local(future, schedule);
            runnable.schedule();
            task
        })
    }

    fn with<T>(&self, f: impl FnOnce() -> T) -> T {
        CURRENT.set(&self, f)
    }

    pub async fn run_until<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        let run_until = RunUntil {
            future,
            local_executor: self,
        };
        run_until.await
    }
}

pin_project! {
    struct RunUntil<'a, F> {
        local_executor: &'a LocalExecutor,
        #[pin]
        future: F,
    }
}

impl<T: Future> Future for RunUntil<'_, T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();
        me.local_executor.with(|| {
            me.local_executor.register(cx.waker());
            if let Poll::Ready(output) = me.future.poll(cx) {
                return Poll::Ready(output);
            }
            if me.local_executor.tick() {
                // If `tick` returns `true`, we need to notify the local future again:
                // there are still tasks remaining in the run queue.
                cx.waker().wake_by_ref();
            }
            Poll::Pending
        })
    }
}
