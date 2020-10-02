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

#[macro_export]
macro_rules! ready {
    ($e:expr $(,)?) => {
        match $e {
            std::task::Poll::Ready(t) => t,
            std::task::Poll::Pending => return core::task::Poll::Pending,
        }
    };
}

pub mod blocking;
pub mod executor;
pub mod io;
pub mod net;
pub mod parking;
pub mod waker_fn;

pub use blocking::block_on;

fn other(msg: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, msg)
}
