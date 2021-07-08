use std::io;
use std::time::Duration;

use slings::net::UdpSocket;
use slings::time::delay_for;

fn main() -> io::Result<()> {
    slings::block_on(async {
        let socket = UdpSocket::bind("127.0.0.1:8081").unwrap();
        println!("udp bind on 127.0.0.1:8081");
        let mut buf = vec![0; 10];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((n, addr)) => {
                    println!("recv bytes {:?} from {}", &buf[..n], addr);
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
