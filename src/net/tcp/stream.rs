use std::net;

#[derive(Debug)]
pub struct TcpStream {
    inner: net::TcpStream,
}

impl TcpStream {
    pub fn from_std(stream: net::TcpStream) -> TcpStream {
        TcpStream { inner: stream }
    }
}
