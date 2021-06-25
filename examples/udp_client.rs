use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use slings::net::UdpSocket;
use slings::runtime::Runtime;
use slings::time::delay_for;

fn main() -> io::Result<()> {
    let runtime = Runtime::new()?;
    runtime.block_on(async {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        println!("local addr: {}", socket.local_addr().unwrap());
        let buf = b"helloworld";
        let addr: SocketAddr = "127.0.0.1:8081".parse().unwrap();
        loop {
            match socket.send_to(buf, addr).await {
                Ok(n) => {
                    println!("send bytes: {:?}", &buf[..n]);
                }
                Err(e) => {
                    println!("read fail {}", e);
                    break;
                }
            }
            delay_for(Duration::from_secs(1)).await;
        }
    });
    Ok(())
}
