use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use slings::net::UdpSocket;
use slings::time::delay_for;

fn main() -> io::Result<()> {
    slings::block_on(async {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        println!("local addr: {}", socket.local_addr()?);
        let addr: SocketAddr = "127.0.0.1:8081".parse().unwrap();
        socket.connect(addr).await?;
        let buf = b"helloworld";
        loop {
            let n = socket.send(buf).await?;
            println!("send bytes: {:?}", &buf[..n]);
            let mut buf = vec![0; 10];
            let n = socket.recv2(&mut buf).await?;
            println!("recv {} bytes", n);
            delay_for(Duration::from_secs(1)).await;
        }
    })
}
