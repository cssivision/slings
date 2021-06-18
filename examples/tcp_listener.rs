use std::io;
use std::net::SocketAddr;

use slings::net::TcpListener;
use slings::runtime::Runtime;
use slings::AsyncWriteExt;

fn main() -> io::Result<()> {
    let runtime = Runtime::new()?;
    runtime.block_on(async {
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let listener = TcpListener::bind(addr).await.unwrap();
        println!("server start listen on 127.0.0.1:8080");

        loop {
            let (mut stream, addr) = listener.accept().await.unwrap();
            println!("accept stream from addr: {:?}", addr);

            let task = slings::spawn_local(async move {
                let buf = b"helloworld";
                stream.write_all(&buf[..]).await.unwrap();
                println!("write 10 bytes");
            });
            task.detach();
        }
    });
    Ok(())
}
