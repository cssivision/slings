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

pub fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    let runtime = Runtime::new().expect("new runtime fail");
    runtime.block_on(future)
}
