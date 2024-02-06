mod mqttdecoder;
use std::fs::File;
use std::io::{self, BufReader};
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
use clap::{App, Arg};
use futures::stream::StreamExt;
use futures::SinkExt;
use pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use rustls_pemfile::{certs, rsa_private_keys};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

//use tokio::io::{AsyncReadExt, AsyncWriteExt};

use std::error::Error;
use tokio::io::split;
use tokio::net::TcpStream;
use tokio_util::codec::{FramedRead, FramedWrite};

type ServerResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
pub struct Config {
    serverconfig: ServerConfig,
    address: SocketAddr,
}

fn load_certs(path: &Path) -> io::Result<Vec<CertificateDer<'static>>> {
    certs(&mut BufReader::new(File::open(path)?)).collect()
}

fn load_keys(path: &Path) -> io::Result<PrivateKeyDer<'static>> {
    rsa_private_keys(&mut BufReader::new(File::open(path)?))
        .next()
        .unwrap()
        .map(Into::into)
}

pub fn run(config: Config) -> ServerResult<()> {
    if let Err(err) = run_main(config) {
        return Err(Box::new(err));
    };
    Ok(())
}

pub fn get_args() -> ServerResult<Config> {
    let matches = App::new("mqtt-server")
        .version("0.0.0")
        .author("Naohiro Heya")
        .about("mqtt server")
        .arg(
            Arg::with_name("addr")
                .value_name("IPADDR")
                .short("a")
                .long("--addr")
                .default_value("127.0.0.1:8883")
                .required(false)
                .help("server's address consist of port")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("cert")
                .value_name("FILEPATH")
                .short("c")
                .long("--cert")
                .default_value("server.crt")
                .help("server cert @ pem format")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("key")
                .value_name("FILEPATH")
                .short("k")
                .long("--key")
                .default_value("private.key")
                .help("server cert @ pem format")
                .takes_value(true),
        )
        .get_matches();

    let certs = load_certs(Path::new(matches.value_of("cert").unwrap()))?;
    let key = load_keys(Path::new(matches.value_of("key").unwrap()))?;
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    let addr = matches.value_of("addr").unwrap();
    let addr: SocketAddr = if let Ok(socketaddr) = addr.parse() {
        socketaddr
    } else {
        return Err(format!("address error, invalid format {}", addr).into());
    };
    Ok(Config {
        serverconfig: config,
        address: addr,
    })
}

#[tokio::main]
async fn run_main(config: Config) -> io::Result<()> {
    let acceptor = TlsAcceptor::from(Arc::new(config.serverconfig));
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
                    mqttdecoder::MQTTPacket::Subscribe(packet) => {
                        println!("Subscribe Packet {:?}", packet);
                        let packet = mqttdecoder::Suback::new(
                            packet.message_id,
                            packet.subscription_list.len(),
                        );
                        let result = frame_writer
                            .send(mqttdecoder::MQTTPacket::Suback(packet))
                            .await;
                        match result {
                            Ok(_) => {
                                println!("Success Suback")
                            }
                            Err(err) => {
                                eprintln!("Error Suback {:?}", err)
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(err) => eprintln!("error: {:?}", err),
        }
    }
    return Ok(());
}
