use std::net::SocketAddr;

use slings::net::TcpListener;

fn main() {
    slings::block_on(async {
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let listener = TcpListener::bind(addr).await.unwrap();

        println!("server start listen on 127.0.0.1:8080");

        loop {
            let (_stream, addr) = listener.accept().await.unwrap();

            println!("addr: {:?}", addr);
        }
    });
}
