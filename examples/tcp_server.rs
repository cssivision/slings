use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use slings::net::TcpListener;
use slings::time::delay_for;
use slings::AsyncWriteExt;

fn main() -> io::Result<()> {
    slings::block_on(async {
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let listener = TcpListener::bind(addr).await.unwrap();
        println!("server start listen on 127.0.0.1:8080");

        loop {
            let (mut stream, addr) = listener.accept().await.unwrap();
            println!("accept stream from addr: {:?}", addr);

            let task = slings::spawn_local(async move {
                loop {
                    let buf = b"helloworld";
                    match stream.write_all(&buf[..]).await {
                        Ok(_) => {
                            println!("write 10 bytes");
                        }
                        Err(e) => {
                            println!("write fail {}", e);
                            break;
                        }
                    }
                    delay_for(Duration::from_secs(1)).await;
                }
            });
            task.detach();
        }
    });
    Ok(())
}
