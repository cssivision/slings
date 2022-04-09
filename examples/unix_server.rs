use std::io;
use std::time::Duration;

use futures_util::AsyncWriteExt;
use slings::net::UnixListener;
use slings::time::delay_for;

fn main() -> io::Result<()> {
    slings::block_on(async {
        let listener = UnixListener::bind("temp.sock")?;
        println!("server start listen on {:?}", listener.local_addr());
        loop {
            let (mut stream, addr) = listener.accept().await?;
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
