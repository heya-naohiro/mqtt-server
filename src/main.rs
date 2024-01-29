mod mqttdecoder;
use std::fs::File;
use std::io::{self, BufReader};
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
use futures::stream::StreamExt;
use futures::SinkExt;
use pki_types::{CertificateDer, PrivateKeyDer};
use rustls_pemfile::{certs, rsa_private_keys};
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
//use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tokio::io::split;
use tokio::net::TcpStream;
use tokio_util::codec::{FramedRead, FramedWrite};

fn load_certs(path: &Path) -> io::Result<Vec<CertificateDer<'static>>> {
    certs(&mut BufReader::new(File::open(path)?)).collect()
}

fn load_keys(path: &Path) -> io::Result<PrivateKeyDer<'static>> {
    rsa_private_keys(&mut BufReader::new(File::open(path)?))
        .next()
        .unwrap()
        .map(Into::into)
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let certs = load_certs(Path::new("server.crt"))?;
    let key = load_keys(Path::new("private.key"))?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:7878").await.unwrap();

    loop {
        let (mut stream, addr) = listener.accept().await.unwrap();
        let acceptor = acceptor.clone();
        if let Err(err) = process_connection(&mut stream, acceptor).await {
            println!("Error process from {:?}, err {:?}", addr, err);
        } else {
            println!("Shutdown process from {:?} Successfully", addr);
        }
    }
}

pub fn add_two(a: i32) -> i32 {
    internal_adder(a, 2)
}

fn internal_adder(a: i32, b: i32) -> i32 {
    a + b
}

async fn process_connection(socket: &mut TcpStream, acceptor: TlsAcceptor) -> io::Result<()> {
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
