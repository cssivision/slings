use std::io;

use slings::net::TcpStream;
use slings::runtime::Runtime;
use slings::AsyncReadExt;

fn main() -> io::Result<()> {
    let runtime = Runtime::new()?;
    runtime.block_on(async {
        let mut stream = TcpStream::connect("127.0.0.1:8080").await.unwrap();
        let mut buf = vec![0; 10];
        stream.read_exact(&mut buf).await.unwrap();
        println!("read bytes: {:?}", buf);
    });
    Ok(())
}
