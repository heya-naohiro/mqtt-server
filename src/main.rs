mod mqttdecoder;

use futures::{prelude::stream::StreamExt, SinkExt};
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
};
use tokio_util::codec::{FramedRead, FramedWrite};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").await.unwrap();

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        process(socket).await;
    }
}

async fn process(mut socket: TcpStream) {
    // Split TcpStream https://zenn.dev/magurotuna/books/tokio-tutorial-ja/viewer/io
    let (rd, wr) = socket.split();
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
                        break;
                    }
                    _ => {}
                }
            }
            Err(err) => eprintln!("error: {:?}", err),
        }
    }
    // disconnect
    let result = socket.shutdown().await;
    match result {
        Ok(()) => println!("TCP stream disconnected successfully"),
        Err(err) => eprintln!("Error disconnecting TCP stream: {}", err),
    }
}
