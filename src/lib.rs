mod cassandra_store;
mod mqttcoder;
mod rpcserver;

use std::fs::File;
use std::io::{self, BufReader};
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
use async_channel::Receiver;
use cdrs_tokio::cluster::session::{Session, SessionBuilder, TcpSessionBuilder};
use cdrs_tokio::load_balancing::RoundRobinLoadBalancingStrategy;
use cdrs_tokio::transport::TransportTcp;
use clap::{App, Arg};
use futures::stream::StreamExt;
use futures::SinkExt;
use pki_types::{CertificateDer, PrivateKeyDer};
use rpcserver::rpcserver::PublishedPacket;
use rustls::ServerConfig;
use rustls_pemfile::{certs, rsa_private_keys};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_rustls::TlsAcceptor;
use tonic::transport::Server;
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::rpcserver::rpcserver::published_packet_service_server::PublishedPacketServiceServer;

use cdrs_tokio::authenticators::NoneAuthenticatorProvider;
use cdrs_tokio::cluster::topology::Node;
use cdrs_tokio::cluster::NodeAddress;
use cdrs_tokio::cluster::{NodeTcpConfigBuilder, TcpConnectionManager};
use std::error::Error;
use tokio::io::split;
use tokio::net::TcpStream;
use tokio_util::codec::{FramedRead, FramedWrite};

type ServerResult<T> = Result<T, Box<dyn Error>>;

type CurrentSession = Session<
    TransportTcp,
    TcpConnectionManager,
    RoundRobinLoadBalancingStrategy<TransportTcp, TcpConnectionManager>,
>;
#[derive(Debug)]
pub struct Config {
    pub serverconfig: ServerConfig,
    pub address: SocketAddr,
    pub cassandra_addr: String,
}

pub fn load_certs(path: &Path) -> io::Result<Vec<CertificateDer<'static>>> {
    certs(&mut BufReader::new(File::open(path)?)).collect()
}

pub fn load_keys(path: &Path) -> io::Result<PrivateKeyDer<'static>> {
    rsa_private_keys(&mut BufReader::new(File::open(path)?))
        .next()
        .unwrap()
        .map(Into::into)
}

pub fn run(config: Config) -> ServerResult<()> {
    // not use now, for test only
    if let Err(err) = start_main(config) {
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
                .help("server key @ pem format")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("cassandra_addr")
                .value_name("CASSANDRA IPADDR")
                .short("db_addr")
                .long("--db_addr")
                .default_value("127.0.0.1:9042")
                .required(false)
                .help("server's address consist of port")
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

    let cassandra_addr = matches.value_of("cassandra_addr").unwrap();
    let cassandra_addr = cassandra_addr;

    Ok(Config {
        serverconfig: config,
        address: addr,
        cassandra_addr: cassandra_addr.to_string(),
    })
}

async fn handle_user_connection(rx: Arc<Receiver<PublishedPacket>>) {
    let addr = "[::1]:10000".parse().unwrap();
    let publish_packet_service = rpcserver::PlatformPublishedPacketService { packet_channel: rx };
    let svc = PublishedPacketServiceServer::new(publish_packet_service);

    let _ = Server::builder().add_service(svc).serve(addr).await;
}

async fn handle_device_connection(config: Arc<Config>) {
    let server_config = config.serverconfig.clone();
    let acceptor = TlsAcceptor::from(Arc::new(server_config));
    let listener = TcpListener::bind(config.address).await.unwrap();
    println!("handle connection");
    loop {
        let (mut stream, addr) = listener.accept().await.unwrap();
        let acceptor = acceptor.clone();
        if let Err(err) = process_connection(&mut stream, acceptor, config.clone()).await {
            println!("Error process from {:?}, err {:?}", addr, err);
        } else {
            println!("Shutdown process from {:?} Successfully", addr);
        }
    }
}

#[tokio::main]
async fn start_main(config: Config) -> io::Result<()> {
    let (mut sender, receiver) = oneshot::channel::<bool>();
    let _ = tokio::spawn(run_main(config, receiver)).await;
    sender.closed().await;
    Ok(())
}

pub async fn run_main(config: Config, receiver: oneshot::Receiver<bool>) -> io::Result<()> {
    let cluster_config = NodeTcpConfigBuilder::new()
        .with_contact_point(config.cassandra_addr.clone().into())
        .with_authenticator_provider(Arc::new(NoneAuthenticatorProvider))
        .build()
        .await
        .unwrap();

    let lb = RoundRobinLoadBalancingStrategy::new();
    let session: Arc<cassandra_store::CurrentSession> =
        Arc::new(TcpSessionBuilder::new(lb, cluster_config).build());
    let _ = match cassandra_store::CassandraStore::initialize(session).await {
        Ok(value) => value,
        Err(e) => {
            println!("Cassandra Error {:?}", e);
        }
    };
    let c = Arc::new(config);
    let c_clone = Arc::clone(&c);

    let (s, r) = async_channel::unbounded();
    let rgrpc = Arc::new(r.clone());
    tokio::select! {
        d = receiver => {
            match d {
                Ok(_) => {
                    println!("Recieve Stop Signal {:?}", d)
                }
                Err(err) => {
                    println!("Recieve Stop Signal {:?}", err)
                }
            }
        },
        _ = handle_device_connection(c_clone) => {
        },
        _ = handle_user_connection(rgrpc) => {

        },
    }
    return Ok(());
}

async fn process_connection(
    socket: &mut TcpStream,
    acceptor: TlsAcceptor,
    config: Arc<Config>,
) -> io::Result<()> {
    if let Err(err) = process(socket, acceptor, config).await {
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

async fn process(
    socket: &mut TcpStream,
    acceptor: TlsAcceptor,
    config: Arc<Config>,
) -> io::Result<()> {
    let mut socket = match acceptor.accept(socket).await {
        Ok(value) => value,
        Err(error) => {
            return Err(error);
        }
    };
    /*
       DB Connection
    */
    /* Create Session */
    let cluster_config = NodeTcpConfigBuilder::new()
        .with_contact_point(config.cassandra_addr.clone().into())
        .with_authenticator_provider(Arc::new(NoneAuthenticatorProvider))
        .build()
        .await
        .unwrap();

    let lb = RoundRobinLoadBalancingStrategy::new();
    let session: Arc<cassandra_store::CurrentSession> =
        Arc::new(TcpSessionBuilder::new(lb, cluster_config).build());

    let stream = &mut socket;
    let (rd, wr) = split(stream);

    let decoder = mqttcoder::MqttDecoder::new();
    let mut frame_reader = FramedRead::new(rd, decoder);
    let encoder = mqttcoder::MqttEncoder::new();
    let mut frame_writer = FramedWrite::new(wr, encoder);
    while let Some(frame) = frame_reader.next().await {
        match frame {
            Ok(data) => {
                println!("received: {:?}", data);
                match data {
                    mqttcoder::MQTTPacket::Connect => {
                        println!("Connect");
                        let packet = mqttcoder::Connack::new();
                        let result = frame_writer
                            .send(mqttcoder::MQTTPacket::Connack(packet))
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
                    mqttcoder::MQTTPacket::Publish(packet) => {
                        println!("Publish Packet {:?}", packet);
                        // [TODO] Data path
                        if let Err(err) = cassandra_store::CassandraStore::store_published_data(
                            session.clone(),
                            packet,
                        )
                        .await
                        {
                            eprintln!("Error DB Store Error {:?}", err)
                        }
                    }
                    mqttcoder::MQTTPacket::Disconnect => {
                        // disconnect
                        break;
                    }
                    mqttcoder::MQTTPacket::Subscribe(packet) => {
                        println!("Subscribe Packet {:?}", packet);
                        let packet = mqttcoder::Suback::new(
                            packet.message_id,
                            packet.subscription_list.len(),
                        );
                        let result = frame_writer
                            .send(mqttcoder::MQTTPacket::Suback(packet))
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
