pub mod action;
pub mod completion;

use std::io;

fn other(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, msg)
}
