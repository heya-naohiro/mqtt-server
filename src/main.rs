mod mqttdecoder;
use futures::stream::StreamExt;
use futures::SinkExt;
use std::fs::File;
use std::io::{self, BufReader};
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
use pki_types::{CertificateDer, PrivateKeyDer};
use rustls_pemfile::{certs, rsa_private_keys};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{copy, sink, split, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;
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
        if let Err(err) = process(&mut stream, acceptor).await {
            println!("Error Shutdown from {:?}, err {:?}", addr, err);
            stream.shutdown().await?;
        } else {
            println!("Shutdown from {:?} Successfully", addr);
            stream.shutdown().await?;
        }
    }
}

async fn process(socket: &mut TcpStream, acceptor: TlsAcceptor) -> io::Result<()> {
    // Split TcpStream https://zenn.dev/magurotuna/books/tokio-tutorial-ja/viewer/io
    let socket = match acceptor.accept(socket).await {
        Ok(value) => value,
        Err(error) => {
            return Err(error);
        }
    };
    let stream = socket;
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
                        break;
                    }
                    _ => {}
                }
            }
            Err(err) => eprintln!("error: {:?}", err),
        }
    }

    // disconnect
    return Ok(());
}
