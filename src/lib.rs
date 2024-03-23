mod cassandra_store;
mod connection_store;

mod mqttcoder;
mod rpcserver;
mod topicfilter;
use std::fs::File;
use std::io::{self, BufReader};
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
use cdrs_tokio::cluster::session::{SessionBuilder, TcpSessionBuilder};
use cdrs_tokio::load_balancing::RoundRobinLoadBalancingStrategy;
use clap::{App, Arg};
use futures::stream::StreamExt;
use futures::SinkExt;
use mqttcoder::MQTTPacket;
use pki_types::{CertificateDer, PrivateKeyDer};
use rpcserver::rpcserver::PublishedPacket;
use rustls::ServerConfig;
use rustls_pemfile::{certs, rsa_private_keys};
use std::net::SocketAddr;
use std::path::Path;
use tokio::io::WriteHalf;
use tokio::io::{AsyncWriteExt, ReadHalf};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_rustls::TlsAcceptor;
use tonic::transport::Server;
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::rpcserver::rpcserver::published_packet_service_server::PublishedPacketServiceServer;
use tokio::sync::broadcast;

use cdrs_tokio::authenticators::NoneAuthenticatorProvider;

use cdrs_tokio::cluster::NodeTcpConfigBuilder;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::io::split;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::codec::{FramedRead, FramedWrite};

type ServerResult<T> = Result<T, Box<dyn Error>>;
type SubscriptionsDB = Arc<Mutex<topicfilter::TopicFilterStore<topicfilter::SubInfo>>>;

#[derive(Debug)]
pub struct ConnectInfo {
    pub connect: mqttcoder::Connect,
    pub sender: mpsc::Sender<MQTTPacket>,
}
impl ConnectInfo {
    pub fn new(connect: mqttcoder::Connect, sender: mpsc::Sender<MQTTPacket>) -> ConnectInfo {
        ConnectInfo {
            connect,
            sender,
            // [TODO] handover sub_filters if clean session is false ?
        }
    }
}

#[derive(Debug)]
pub struct Subfilter {
    pub filter: Filter,
    pub qos: u8,
}

#[derive(Debug)]
pub struct Filter {
    // all 8bit str
    pub elements: Vec<u8>,
}

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

async fn handle_user_connection(
    reciever: broadcast::Receiver<PublishedPacket>,
    connection_map: connection_store::ConnectionStateDB,
) {
    let addr = "[::1]:10000".parse().unwrap();
    let publish_packet_service =
        rpcserver::PlatformPublishedPacketService::new(reciever, connection_map);
    let svc = PublishedPacketServiceServer::new(publish_packet_service);

    let _ = Server::builder().add_service(svc).serve(addr).await;
}

async fn handle_device_connection(
    config: Arc<Config>,
    sender: broadcast::Sender<PublishedPacket>,
    connection_map: connection_store::ConnectionStateDB,
) {
    let server_config = config.serverconfig.clone();
    let acceptor = TlsAcceptor::from(Arc::new(server_config));
    let listener = TcpListener::bind(config.address).await.unwrap();
    let subscription_store = Arc::new(Mutex::new(topicfilter::TopicFilterStore::new()));

    println!("handle connection");
    loop {
        println!("handle connection1");
        let (mut stream, addr) = listener.accept().await.unwrap();
        let acceptor = acceptor.clone();
        let config = config.clone();
        let sender = sender.clone();
        let connection_map = connection_map.clone();
        let subscription_store = subscription_store.clone();
        tokio::spawn(async move {
            if let Err(err) = process_connection(
                &mut stream,
                acceptor,
                config,
                sender,
                connection_map,
                subscription_store,
            )
            .await
            {
                println!("Error process from {:?}, err {:?}", addr, err);
            }
        });
        /*
        if let Err(err) = process_connection(
            &mut stream,
            acceptor,
            config.clone(),
            sender.clone(),
            connection_map.clone(),
        )
        .await
        {
            println!("Error process from {:?}, err {:?}", addr, err);
        } else {
            println!("Shutdown process from {:?} Successfully", addr);
        }
        */
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
    println!("Run main");

    let cluster_config = NodeTcpConfigBuilder::new()
        .with_contact_point(config.cassandra_addr.clone().into())
        .with_authenticator_provider(Arc::new(NoneAuthenticatorProvider))
        .build()
        .await
        .unwrap();
    println!("Run main 2");

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
    println!("Run main3");

    // Connection Informationの共有
    let connection_db = Arc::new(Mutex::new(HashMap::new()));
    let connection_db_2 = connection_db.clone();
    // mqttとgRPCをつなぐ
    let (s, r) = broadcast::channel::<PublishedPacket>(10);
    {
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
            _ = handle_device_connection(c_clone, s, connection_db) => {
            },
            _ = handle_user_connection(r, connection_db_2) => {

            },
        }
    }
    return Ok(());
}

async fn process_connection(
    socket: &mut TcpStream,
    acceptor: TlsAcceptor,
    config: Arc<Config>,
    sender: broadcast::Sender<PublishedPacket>,
    connection_map: connection_store::ConnectionStateDB,
    subscription_store: SubscriptionsDB,
) -> io::Result<()> {
    // Subscrptionの共有
    println!("Process Connection");

    if let Err(err) = process(
        socket,
        acceptor,
        config,
        sender,
        connection_map,
        subscription_store,
    )
    .await
    {
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

async fn send_packet(
    mut frame_writer: FramedWrite<
        WriteHalf<tokio_rustls::server::TlsStream<&mut TcpStream>>,
        mqttcoder::MqttEncoder,
    >,
    mut rx: mpsc::Receiver<MQTTPacket>,
) -> io::Result<()> {
    while let Some(packet) = rx.recv().await {
        let result = frame_writer.send(packet).await;
        match result {
            Ok(_) => {
                println!("Success Send Packet")
            }
            Err(err) => {
                eprintln!("Error Send Packet {:?}", err)
            }
        }
    }
    return Ok(());
}

async fn recv_packet(
    mut frame_reader: FramedRead<
        ReadHalf<tokio_rustls::server::TlsStream<&mut TcpStream>>,
        mqttcoder::MqttDecoder,
    >,
    connection_map: connection_store::ConnectionStateDB,
    tx: mpsc::Sender<MQTTPacket>,
    config: Arc<Config>,
    sender: broadcast::Sender<PublishedPacket>,
    subscription_store: SubscriptionsDB,
) -> io::Result<()> {
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

    // 今処理をしているclient id
    let mut client_id: Option<String> = None;
    while let Some(frame) = frame_reader.next().await {
        match frame {
            Ok(data) => {
                println!("received: {:?}", data);
                match data {
                    mqttcoder::MQTTPacket::Connect(packet) => {
                        println!("Connect");
                        /* Store Hashmap */
                        let mut cmap = connection_map.lock().await;
                        /* [TODO] Need Check Client id already exist, close previous session  */
                        let client_id_2 = packet.client_id.clone();

                        cmap.insert(
                            packet.client_id.clone(),
                            connection_store::ConnectInfo::new(packet, tx.clone()),
                        );
                        client_id = Some(client_id_2);
                        let packet = mqttcoder::Connack::new();
                        if let Err(err) = tx.send(mqttcoder::MQTTPacket::Connack(packet)).await {
                            return Err(io::Error::new(io::ErrorKind::Other, err));
                        }
                    }
                    mqttcoder::MQTTPacket::Publish(packet) => {
                        println!("Publish Packet {:?}", packet);
                        // [TODO] Data path
                        if let Err(err) = cassandra_store::CassandraStore::store_published_data(
                            session.clone(),
                            packet.clone(),
                        )
                        .await
                        {
                            eprintln!("Error DB Store Error {:?}", err)
                        }
                        /* broker send */
                        {
                            let mut subscription_store_guard = subscription_store.lock().await;

                            let l = match subscription_store_guard
                                .get_topicfilter(&packet.topic_name)
                            {
                                Err(err) => {
                                    eprintln!("Error broker get topic {}", err);
                                    return Err(err);
                                }
                                Ok(r) => r,
                            };

                            let mut garbage_ids = vec![];
                            for s in l {
                                match &s.sender {
                                    Some(sender) => {
                                        // packet copy on memory for multi subscribers
                                        if let Err(err) =
                                            sender.send(MQTTPacket::Publish(packet.clone())).await
                                        {
                                            garbage_ids.push(s.client_id.clone());
                                            eprintln!(
                                                "Error send publilsh packet device broker {}",
                                                err
                                            );
                                        }
                                    }
                                    None => eprintln!("Error: Not Exist sender"),
                                }
                            }
                            // remove pipe broken subscriber
                            for gid in garbage_ids {
                                subscription_store_guard.remove_subscription(&gid);
                            }
                        }

                        /* user send */
                        let send_packet = PublishedPacket {
                            device_id: "device".to_string(),
                            topic: packet.topic_name,
                            payload: packet.payload,
                        };
                        let result = sender.send(send_packet);
                        match result {
                            Err(err) => eprintln!("Packet broadcast Error {}", err),
                            Ok(size) => println!("Packet broadcast OK {}", size),
                        }
                    }
                    mqttcoder::MQTTPacket::Disconnect => {
                        // disconnect
                        // gracefull shutdown
                        let mut subscription_store = subscription_store.lock().await;

                        let client_id = match client_id {
                            Some(ref client_id) => client_id,
                            None => {
                                eprint!("Client id is not found");
                                return Err(io::Error::new(
                                    io::ErrorKind::Other,
                                    "Client id is not found",
                                ));
                            }
                        };
                        subscription_store.remove_subscription(client_id);
                        break;
                    }
                    mqttcoder::MQTTPacket::Subscribe(packet) => {
                        {
                            let mut subscription_store = subscription_store.lock().await;
                            for packet in &packet.subscription_list {
                                println!("Register topic filter");
                                // client_id check
                                let client_id = match client_id {
                                    Some(ref client_id) => client_id,
                                    None => {
                                        eprint!("Client id is not found");
                                        return Err(io::Error::new(
                                            io::ErrorKind::Other,
                                            "Client id is not found",
                                        ));
                                    }
                                };

                                if let Err(err) = subscription_store.register_topicfilter(
                                    topicfilter::SubInfo::new(
                                        packet.topicfilter.clone(),
                                        Some(tx.clone()),
                                        client_id.to_string(),
                                    ),
                                    client_id.to_string(),
                                ) {
                                    eprint!("Error subscription {:?}", err);
                                }
                            }
                        }

                        let packet = mqttcoder::Suback::new(
                            packet.message_id,
                            packet.subscription_list.len(),
                        );
                        // [TODO] Error handling
                        if let Err(err) = tx.send(mqttcoder::MQTTPacket::Suback(packet)).await {
                            return Err(io::Error::new(io::ErrorKind::Other, err));
                        };
                    }
                    mqttcoder::MQTTPacket::Pingreq(_packet) => {
                        // ping / pingresp
                        println!("Recv Ping req");
                        let packet = mqttcoder::Pingresp::new();
                        if let Err(err) = tx.send(mqttcoder::MQTTPacket::Pingresp(packet)).await {
                            return Err(io::Error::new(io::ErrorKind::Other, err));
                        };
                        println!("Ping res");
                    }

                    _ => {}
                }
            }
            Err(err) => eprintln!("error: {:?}", err),
        }
    }
    return Ok(());
}

async fn process(
    socket: &mut TcpStream,
    acceptor: TlsAcceptor,
    config: Arc<Config>,
    sender: broadcast::Sender<PublishedPacket>,
    connection_map: connection_store::ConnectionStateDB,
    subscription_map: SubscriptionsDB,
) -> io::Result<()> {
    println!("process");
    let socket = match acceptor.accept(socket).await {
        Ok(value) => value,
        Err(error) => {
            return Err(error);
        }
    };

    let (rd, wr) = split(socket);

    let decoder = mqttcoder::MqttDecoder::new();
    let frame_reader = FramedRead::new(rd, decoder);
    let encoder = mqttcoder::MqttEncoder::new();

    let frame_writer = FramedWrite::new(wr, encoder);

    let (tx, rx) = mpsc::channel(32);

    tokio::select! {
        _ = send_packet(frame_writer, rx) => {

        }
        _ = recv_packet(
            frame_reader,
            connection_map,
            tx,
            config,
            sender, subscription_map) => {

            }
        /* [TODO] cancel from circuit breaker */
    }
    return Ok(());
}
