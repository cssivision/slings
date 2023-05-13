use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use futures_util::AsyncWriteExt;
use slings::net::TcpListener;
use slings::time::delay_for;

fn main() -> io::Result<()> {
    slings::block_on(async {
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let listener = TcpListener::bind(addr)?;
        println!("server start listen on {:?}", listener.local_addr());
        loop {
            let (mut stream, addr) = listener.accept2().await?;
            println!("accept stream from addr: {:?}", addr);
            slings::spawn_local(async move {
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
            })
            .detach();
        }
    })
}
