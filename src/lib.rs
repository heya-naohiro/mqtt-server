mod mqttdecoder;

use futures::stream::StreamExt;
use futures::SinkExt;
use std::io::{self};
//use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tokio::io::{split, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsAcceptor;
use tokio_util::codec::{FramedRead, FramedWrite};

pub fn add_two(a: i32) -> i32 {
    internal_adder(a, 2)
}

fn internal_adder(a: i32, b: i32) -> i32 {
    a + b
}

pub async fn process_connection(socket: &mut TcpStream, acceptor: TlsAcceptor) -> io::Result<()> {
    if let Err(err) = process(socket, acceptor).await {
        println!("Error process from err {:?}", err);
        socket.shutdown().await?;
        return Err(err);
    } else {
        println!("Shutdown process from Successfully");
        socket.flush().await?;
        socket.shutdown().await?;
    }
    return Ok(());
}

async fn process(socket: &mut TcpStream, acceptor: TlsAcceptor) -> io::Result<()> {
    let mut socket = match acceptor.accept(socket).await {
        Ok(value) => value,
        Err(error) => {
            return Err(error);
        }
    };

    let stream = &mut socket;
    let (rd, wr) = split(stream);

    let decoder = mqttdecoder::MqttDecoder::new();
    let mut frame_reader = FramedRead::new(rd, decoder);
    let encoder = mqttdecoder::MqttEncoder::new();
    let mut frame_writer = FramedWrite::new(wr, encoder);
    while let Some(frame) = frame_reader.next().await {
        match frame {
            Ok(data) => {
                println!("received: {:?}", data);
                match data {
                    mqttdecoder::MQTTPacket::Connect => {
                        println!("Connect");
                        let packet = mqttdecoder::Connack::new();
                        let result = frame_writer
                            .send(mqttdecoder::MQTTPacket::Connack(packet))
                            .await;
                        match result {
                            Ok(_) => {
                                println!("Success Connack")
                            }
                            Err(err) => {
                                eprintln!("Error Connack {:?}", err)
                            }
                        }
                    }
                    mqttdecoder::MQTTPacket::Publish(packet) => {
                        println!("Publish Packet {:?}", packet);
                    }
                    mqttdecoder::MQTTPacket::Disconnect => {
                        // disconnect
                        break;
                    }
                    _ => {}
                }
            }
            Err(err) => eprintln!("error: {:?}", err),
        }
    }
    return Ok(());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal() {
        assert_eq!(4, internal_adder(2, 2));
    }
}
