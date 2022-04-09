use std::io;
use std::time::Duration;

use futures_util::AsyncReadExt;
use slings::net::UnixStream;
use slings::time::delay_for;

fn main() -> io::Result<()> {
    slings::block_on(async {
        let mut stream = UnixStream::connect("temp.sock").await?;
        let mut buf = vec![0; 10];
        loop {
            stream.read_exact(&mut buf).await?;
            println!("read bytes: {:?}", buf);
            delay_for(Duration::from_secs(1)).await;
        }
    })
}
