use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use slings::net::UdpSocket;
use slings::time::delay_for;

fn main() -> io::Result<()> {
    slings::block_on(async {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        println!("local addr: {}", socket.local_addr()?);
        let buf = b"helloworld";
        let addr: SocketAddr = "127.0.0.1:8081".parse().unwrap();
        loop {
            let n = socket.send_to(buf, addr).await?;
            println!("send bytes: {:?}", &buf[..n]);
            delay_for(Duration::from_secs(1)).await;
        }
    })
}
