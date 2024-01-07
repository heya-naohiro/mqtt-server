extern crate hello;
use hello::ThreadPool;

use mqtt::control::ConnectReturnCode;
use mqtt::packet::{ConnackPacket, VariablePacket};
use mqtt::Decodable;
use mqtt::Encodable;
use std::io::prelude::*;
use std::io::Cursor;
use std::net::TcpListener;
use std::net::TcpStream;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    let pool = ThreadPool::new(4);
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        println!("Connection established!");

        pool.execute(|| {
            handle_connection(stream);
        });
    }
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).unwrap();

    let mut dec_buf = Cursor::new(&buffer[..]);
    let auto_decode = VariablePacket::decode(&mut dec_buf).unwrap();
    println!("Variable packet decode: {:?}", auto_decode);
    let mut buf = Vec::new();
    match auto_decode {
        VariablePacket::ConnectPacket(_) => {
            println!("Connect Packet {:?}", auto_decode);
            let packet = ConnackPacket::new(false, ConnectReturnCode::ConnectionAccepted);
            packet.encode(&mut buf).unwrap();
        }
        _ => {
            println!("Other Packet {:?}", auto_decode);
        }
    }

    stream.write(&buf).unwrap();
    stream.flush().unwrap();
}
