use std::io;
use std::time::Duration;

use slings::net::TcpStream;
use slings::time::delay_for;
use slings::AsyncReadExt;

fn main() -> io::Result<()> {
    slings::block_on(async {
        let mut stream = TcpStream::connect("127.0.0.1:8080").await.unwrap();
        let mut buf = vec![0; 10];
        loop {
            match stream.read_exact(&mut buf).await {
                Ok(_) => {
                    println!("read bytes: {:?}", buf);
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
