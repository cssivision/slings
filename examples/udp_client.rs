use std::io;
use std::time::Duration;

use slings::net::UdpSocket;
use slings::runtime::Runtime;
use slings::time::delay_for;

fn main() -> io::Result<()> {
    let runtime = Runtime::new()?;
    runtime.block_on(async {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        println!("local addr: {}", socket.local_addr().unwrap());
        socket.connect("127.0.0.1:8081").unwrap();
        let buf = b"helloworld";
        loop {
            match socket.send(buf).await {
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
