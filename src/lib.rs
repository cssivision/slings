#[macro_export]
macro_rules! pin_mut {
    ($($x:ident),* $(,)?) => { $(
        // Move the value to ensure that it is owned
        let mut $x = $x;
        // Shadow the original binding so that it can't be directly accessed
        // ever again.
        #[allow(unused_mut)]
        let mut $x = unsafe {
            std::pin::Pin::new_unchecked(&mut $x)
        };
    )* }
}

#[allow(unused_macros)]
macro_rules! syscall {
    ($fn: ident ( $($arg: expr),* $(,)* ) ) => {{
        let res = unsafe { libc::$fn($($arg, )*) };
        if res == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res)
        }
    }};
}

#[macro_export]
macro_rules! ready {
    ($e:expr $(,)?) => {
        match $e {
            std::task::Poll::Ready(t) => t,
            std::task::Poll::Pending => return core::task::Poll::Pending,
        }
    };
}

mod driver;
mod local_executor;
pub mod net;
pub mod runtime;
mod socket;
pub mod time;
mod waker_fn;

use std::future::Future;

pub use local_executor::spawn_local;
pub use runtime::Runtime;

pub use async_task::Task;
pub use futures_util::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    let runtime = Runtime::new().expect("new runtime fail");
    runtime.block_on(future)
}
