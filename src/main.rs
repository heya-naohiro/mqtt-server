use mqtt::control::ConnectReturnCode;
use mqtt::packet::{ConnackPacket, VariablePacket};
use mqtt::Decodable;
use mqtt::Encodable;
use std::error::Error;
use std::io::Cursor;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:7878").await?;
    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let mut buf = vec![0; 1024];
            loop {
                let n = socket
                    .read(&mut buf)
                    .await
                    .expect("failed to read data from socket");
                if n == 0 {
                    return;
                }
                /* Processing */
                buf = handle_tcp_packet(&mut buf);

                socket
                    .write_all(&buf[0..])
                    .await
                    .expect("failed to write data to socket");
            }
        });
    }
}

fn handle_tcp_packet(buf: &mut Vec<u8>) -> Vec<u8> {
    let mut dec_buf = Cursor::new(&buf);
    let auto_decode = VariablePacket::decode(&mut dec_buf).unwrap();
    let mut buf = Vec::new();
    match auto_decode {
        VariablePacket::ConnectPacket(_) => {
            let packet = ConnackPacket::new(false, ConnectReturnCode::ConnectionAccepted);
            packet.encode(&mut buf).unwrap();
        }
        VariablePacket::PublishPacket(pubpacket) => {
            println!("Publish Packet {:?}", pubpacket);
        }
        VariablePacket::DisconnectPacket(_) => {
            println!("Close Packet {:?}", auto_decode);
        }
        VariablePacket::PingreqPacket(_) => {
            println!("Ping Packet {:?}", auto_decode);
            auto_decode.encode(&mut buf).unwrap();
        }
        _ => {
            println!("Other Packet {:?}", auto_decode);
        }
    }
    return buf;
}

/*
fn handle_tcp_packet(mut stream: TcpStream) {
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
        VariablePacket::DisconnectPacket(_) => {
            println!("Close Packet {:?}", auto_decode);
        }
        VariablePacket::PingreqPacket(_) => {
            println!("Ping Packet {:?}", auto_decode);
            auto_decode.encode(&mut buf).unwrap();
        }
        VariablePacket::PublishPacket(pubpacket) => {
            println!("Publish Packet {:?}", pubpacket);
        }
        _ => {
            println!("Other Packet {:?}", auto_decode);
        }
    }

    stream.write(&buf).unwrap();
    stream.flush().unwrap();
}
*/
