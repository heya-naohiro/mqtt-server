mod mqttdecoder;

use futures::prelude::stream::StreamExt;
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::FramedRead;
#[tokio::main]
async fn main() {
    // リスナーをこのアドレスにバインドする
    let listener = TcpListener::bind("127.0.0.1:7878").await.unwrap();

    loop {
        // タプルの2つ目の要素は、新しいコネクションのIPとポートの情報を含んでいる
        let (socket, _) = listener.accept().await.unwrap();
        process(socket).await;
    }
}

async fn process(socket: TcpStream) {
    let decoder = mqttdecoder::MqttDecoder::new();
    let mut frame_reader = FramedRead::new(socket, decoder);
    while let Some(frame) = frame_reader.next().await {
        match frame {
            Ok(data) => println!("received: {:?}", data),
            Err(err) => eprintln!("error: {:?}", err),
        }
    }
}
