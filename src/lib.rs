mod cassandra_store;
mod connection_store;
use tracing;
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

use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_rustls::TlsAcceptor;
use tonic::transport::Server;
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::rpcserver::rpcserver::published_packet_service_server::PublishedPacketServiceServer;
use tokio::sync::broadcast;
use tracing::Level;
use tracing::{debug, error, info, trace, warn};

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
type RetainPacketDB = Arc<Mutex<HashMap<String, mqttcoder::Publish>>>;

#[derive(Debug)]
enum LogLevel {
    DEBUG,
    TRACE,
    INFO,
}

#[derive(Debug)]
pub struct ConnectInfo {
    pub connect: mqttcoder::Connect,
    pub sender: mpsc::UnboundedSender<MQTTPacket>,
}
impl ConnectInfo {
    pub fn new(
        connect: mqttcoder::Connect,
        sender: mpsc::UnboundedSender<MQTTPacket>,
    ) -> ConnectInfo {
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
    pub brokermode: bool,
    pub tls: bool,
    pub loglevel: LogLevel,
}

#[tracing::instrument(level = "trace")]
pub fn load_certs(path: &Path) -> io::Result<Vec<CertificateDer<'static>>> {
    certs(&mut BufReader::new(File::open(path)?)).collect()
}
#[tracing::instrument(level = "trace")]
pub fn load_keys(path: &Path) -> io::Result<PrivateKeyDer<'static>> {
    rsa_private_keys(&mut BufReader::new(File::open(path)?))
        .next()
        .unwrap()
        .map(Into::into)
}

#[tracing::instrument(level = "trace")]
pub fn run(config: Config) -> ServerResult<()> {
    // log setting
    let l = match config.loglevel {
        LogLevel::TRACE => Level::TRACE,
        LogLevel::DEBUG => Level::DEBUG,
        _ => Level::INFO,
    };
    tracing_subscriber::fmt().with_max_level(l).init();
    info!("Loglevel {:?}", config.loglevel);

    if let Err(err) = start_main(config) {
        return Err(Box::new(err));
    };
    Ok(())
}

#[tracing::instrument(level = "trace")]
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
                .long("--db_addr")
                .default_value("")
                .required(false)
                .help("This is the port for Cassandra used to store the latest topics. If not specified, Cassandra will not be used.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("non_broker")
                .value_name("Non Broker mode")
                .short("n")
                .long("--non-broker")
                .required(false)
                .help("server start in a mode where communication with clients occurs exclusively via gRPC, not through an MQTT broker, and direct communication between clients is not possible.")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("non_tls")
                .value_name("Non tls mode")
                .long("--non-tls")
                .required(false)
                .help("non tls mode")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("debug")
                .value_name("for debug")
                .long("--debug")
                .required(false)
                .help("log level debug")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("trace")
                .value_name("for trace")
                .long("--trace")
                .required(false)
                .help("log level trace")
                .takes_value(false),
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
    let brokermode = !matches.is_present("non_broker");
    let tls = !matches.is_present("non_tls");
    let loglevel = if matches.is_present("trace") {
        LogLevel::TRACE
    } else if matches.is_present("debug") {
        LogLevel::DEBUG
    } else {
        LogLevel::INFO
    };
    let cassandra_addr = matches.value_of("cassandra_addr").unwrap();
    let cassandra_addr = cassandra_addr;
    Ok(Config {
        serverconfig: config,
        address: addr,
        cassandra_addr: cassandra_addr.to_string(),
        brokermode,
        tls,
        loglevel,
    })
}

#[tracing::instrument(level = "trace")]
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

    // Retain用にHashMapを共有する [TODO] HashMapでは良くないので構造化する
    let retain_store = Arc::new(Mutex::new(HashMap::new()));

    if config.tls {
        info!("MQTT TLS Listener Running");
        loop {
            let (mut stream, addr) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            let config = config.clone();
            let sender = sender.clone();
            let connection_map = connection_map.clone();
            let subscription_store = subscription_store.clone();
            let retain_store = retain_store.clone();
            tokio::spawn(async move {
                if let Err(err) = process_connection_tls(
                    &mut stream,
                    acceptor,
                    config,
                    sender,
                    connection_map,
                    subscription_store,
                    retain_store,
                )
                .await
                {
                    error!("Error process from {:?}, err {:?}", addr, err);
                }
            });
        }
    } else {
        info!("MQTT TCP Listener Running");
        loop {
            let (mut stream, addr) = listener.accept().await.unwrap();
            let config = config.clone();
            let sender = sender.clone();
            let connection_map = connection_map.clone();
            let subscription_store = subscription_store.clone();
            let retain_store = retain_store.clone();
            tokio::spawn(async move {
                if let Err(err) = process_connection_tcp(
                    &mut stream,
                    config,
                    sender,
                    connection_map,
                    subscription_store,
                    retain_store,
                )
                .await
                {
                    error!("Error process from {:?}, err {:?}", addr, err);
                }
            });
        }
    }
}

#[tokio::main]
async fn start_main(config: Config) -> io::Result<()> {
    info!("start tokio main");
    //console_subscriber::init();
    let (mut sender, receiver) = oneshot::channel::<bool>();
    let _ = tokio::spawn(run_main(config, receiver)).await;
    sender.closed().await;
    Ok(())
}

pub async fn run_main(config: Config, receiver: oneshot::Receiver<bool>) -> io::Result<()> {
    if config.cassandra_addr != "" {
        let cassandra_cluster_config = NodeTcpConfigBuilder::new()
            .with_contact_point(config.cassandra_addr.clone().into())
            .with_authenticator_provider(Arc::new(NoneAuthenticatorProvider))
            .build()
            .await
            .unwrap();

        let lb = RoundRobinLoadBalancingStrategy::new();
        let session: Arc<cassandra_store::CurrentSession> =
            Arc::new(TcpSessionBuilder::new(lb, cassandra_cluster_config).build());

        info!("cassandra session initializing...");
        let _ = match cassandra_store::CassandraStore::initialize(session).await {
            Ok(value) => value,
            Err(e) => {
                error!("Cassandra Error {:?}", e);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "cassandra initialize error".to_string(),
                ));
            }
        };
        info!("cassandra session initialize sucess");
    }

    let c = Arc::new(config);
    let c_clone = Arc::clone(&c);

    // Connection Informationの共有
    let connection_db = Arc::new(Mutex::new(HashMap::new()));
    let connection_db_2 = connection_db.clone();
    // mqttとgRPCをつなぐ
    let (s, r) = broadcast::channel::<PublishedPacket>(10);
    info!("mqtt server start");
    {
        tokio::select! {
            d = receiver => {
                match d {
                    Ok(_) => {
                        info!("Recieve Stop Signal {:?}", d)
                    }
                    Err(err) => {
                        warn!("Recieve Stop Signal {:?}", err)
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

async fn process_connection_tls(
    socket: &mut TcpStream,
    acceptor: TlsAcceptor,
    config: Arc<Config>,
    sender: broadcast::Sender<PublishedPacket>,
    connection_map: connection_store::ConnectionStateDB,
    subscription_store: SubscriptionsDB,
    retain_store: RetainPacketDB,
) -> io::Result<()> {
    // Subscrptionの共有
    if let Err(err) = process_tls(
        socket,
        acceptor,
        config,
        sender,
        connection_map,
        subscription_store,
        retain_store,
    )
    .await
    {
        error!("Error process from err {:?}", err);
        socket.shutdown().await?;
        return Err(err);
    } else {
        //debug!("Shutdown process from Successfully");
        socket.flush().await?;
        socket.shutdown().await?;
    }
    return Ok(());
}

async fn process_connection_tcp(
    socket: &mut TcpStream,
    config: Arc<Config>,
    sender: broadcast::Sender<PublishedPacket>,
    connection_map: connection_store::ConnectionStateDB,
    subscription_store: SubscriptionsDB,
    retain_store: RetainPacketDB,
) -> io::Result<()> {
    // Subscrptionの共有
    if let Err(err) = process_tcp(
        socket,
        config,
        sender,
        connection_map,
        subscription_store,
        retain_store,
    )
    .await
    {
        error!("Error process from err {:?}", err);
        socket.shutdown().await?;
        return Err(err);
    } else {
        //debug!("Shutdown process from Successfully");
        socket.flush().await?;
        socket.shutdown().await?;
    }
    return Ok(());
}

#[tracing::instrument(level = "trace")]
async fn send_packet<T>(
    mut frame_writer: FramedWrite<T, mqttcoder::MqttEncoder>,
    mut rx: mpsc::UnboundedReceiver<MQTTPacket>,
) -> io::Result<()>
where
    T: std::fmt::Debug,
    FramedWrite<T, mqttcoder::MqttEncoder>:
        futures::Sink<MQTTPacket, Error = io::Error> + std::fmt::Debug + std::marker::Unpin,
{
    while let Some(packet) = rx.recv().await {
        let result = frame_writer.send(packet).await;
        match result {
            Ok(_) => {
                debug!("Success Send Packet")
            }
            Err(err) => {
                error!("Error Send Packet: {:?}", err)
            }
        }
    }
    return Ok(());
}

#[tracing::instrument(level = "trace")]
async fn recv_packet<T>(
    mut frame_reader: FramedRead<T, mqttcoder::MqttDecoder>,
    connection_map: connection_store::ConnectionStateDB,
    tx: mpsc::UnboundedSender<MQTTPacket>,
    config: Arc<Config>,
    sender: broadcast::Sender<PublishedPacket>,
    subscription_store: SubscriptionsDB,
    retain_store: RetainPacketDB,
) -> io::Result<()>
where
    T: std::fmt::Debug,
    FramedRead<T, mqttcoder::MqttDecoder>:
        futures::Stream<Item = Result<MQTTPacket, io::Error>> + std::marker::Unpin,
{
    /*
       DB Connection
    */
    /* Create Session */
    let mut cassandra_session: Option<Arc<cassandra_store::CurrentSession>> = None;
    if config.cassandra_addr != "" {
        let cassandra_cluster_config = NodeTcpConfigBuilder::new()
            .with_contact_point(config.cassandra_addr.clone().into())
            .with_authenticator_provider(Arc::new(NoneAuthenticatorProvider))
            .build()
            .await
            .unwrap();
        let lb = RoundRobinLoadBalancingStrategy::new();
        cassandra_session = Some(Arc::new(
            TcpSessionBuilder::new(lb, cassandra_cluster_config).build(),
        ));
    }

    // 今処理をしているclient id
    let mut client_id: Option<String> = None;
    while let Some(frame) = frame_reader.next().await {
        match frame {
            Ok(data) => {
                debug!("received: {:?}", data);
                match data {
                    mqttcoder::MQTTPacket::Connect(packet) => {
                        //debug!("Connect {:?}", packet);
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
                        if let Err(err) = tx.send(mqttcoder::MQTTPacket::Connack(packet)) {
                            return Err(io::Error::new(io::ErrorKind::Other, err));
                        }
                    }
                    mqttcoder::MQTTPacket::Publish(packet) => {
                        //debug!("Packet::Publish {:?}", packet);
                        // [TODO] Data path
                        match cassandra_session {
                            Some(ref s) => {
                                if let Err(err) =
                                    cassandra_store::CassandraStore::store_published_data(
                                        s.clone(),
                                        packet.clone(),
                                    )
                                    .await
                                {
                                    error!("Error DB Store Error {:?}", err)
                                }
                            }
                            None => {
                                // skip
                            }
                        }

                        /* broker send */
                        if config.brokermode {
                            debug!("Published Packet: {:?}", packet);
                            let mut subscription_store_guard = subscription_store.lock().await;
                            let l = match subscription_store_guard
                                .get_topicfilter(&packet.topic_name)
                            {
                                Err(err) => {
                                    error!("Error broker get topic {}", err);
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
                                            sender.send(MQTTPacket::Publish(packet.clone()))
                                        {
                                            garbage_ids.push(s.client_id.clone());
                                            error!(
                                                "Error send publilsh packet device broker {}",
                                                err
                                            );
                                        }
                                    }
                                    None => error!("Error: Not Exist sender"),
                                }
                            }
                            // remove pipe broken subscriber
                            for gid in garbage_ids {
                                subscription_store_guard.remove_subscription(&gid);
                            }
                        }
                        /* save the packet for retain */
                        if packet.retain {
                            let mut retain_store_guard = retain_store.lock().await;
                            retain_store_guard.insert(packet.topic_name.clone(), packet.clone());
                        }

                        /* user send */
                        let send_packet = PublishedPacket {
                            device_id: "device".to_string(),
                            topic: packet.topic_name,
                            payload: packet.payload,
                        };
                        let result = sender.send(send_packet);
                        match result {
                            Err(err) => error!("Packet broadcast Error {}", err),
                            Ok(size) => trace!("Packet broadcast OK {}", size),
                        }
                    }
                    mqttcoder::MQTTPacket::Disconnect => {
                        //debug!("Disconnect");
                        // disconnect
                        // gracefull shutdown
                        let mut subscription_store = subscription_store.lock().await;

                        let client_id_4 = match client_id {
                            Some(cid) => cid,
                            None => {
                                warn!("Client id is not found");
                                return Err(io::Error::new(
                                    io::ErrorKind::Other,
                                    "Client id is not found",
                                ));
                            }
                        };
                        subscription_store.remove_subscription(&client_id_4);
                        break;
                    }
                    mqttcoder::MQTTPacket::Unsubscribe(packet) => {
                        //let mut subscription_store = subscription_store.lock().await;
                        // [TODO]
                        //for packet in &packet.unsubscription_list {}
                    }
                    mqttcoder::MQTTPacket::Subscribe(packet) => {
                        {
                            let mut subscription_store = subscription_store.lock().await;
                            for packet in &packet.subscription_list {
                                // client_id check
                                let client_id_3 = match &client_id {
                                    Some(client_id) => client_id,
                                    None => {
                                        error!("Client id is not found");
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
                                        client_id_3.to_string(),
                                    ),
                                    client_id_3.to_string(),
                                ) {
                                    error!("Error subscription {:?}", err);
                                }
                            }
                        }

                        let packet = mqttcoder::Suback::new(
                            packet.message_id,
                            packet.subscription_list.len(),
                        );
                        // [TODO] Error handling
                        if let Err(err) = tx.send(mqttcoder::MQTTPacket::Suback(packet)) {
                            return Err(io::Error::new(io::ErrorKind::Other, err));
                        };

                        /* retain processing ... */
                        /* [TODO] reduce algorithmic complexity and retain may be good, another thread? */
                        {
                            let retain_store_guard = retain_store.lock().await;
                            let mut subscription_store_guard = subscription_store.lock().await;

                            for (topic, packet) in retain_store_guard.iter() {
                                let l = match subscription_store_guard.get_topicfilter(topic) {
                                    Err(err) => {
                                        error!("Error broker get topic {}", err);
                                        return Err(err);
                                    }
                                    Ok(r) => r,
                                };

                                let mut garbage_ids = vec![];
                                let mut client_3 = client_id.clone();
                                for s in l {
                                    // [TODO] Refine structure
                                    if s.client_id == client_3.take().unwrap() {
                                        match &s.sender {
                                            Some(sender) => {
                                                // packet copy on memory for multi subscribers
                                                if let Err(err) =
                                                    sender.send(MQTTPacket::Publish(packet.clone()))
                                                {
                                                    garbage_ids.push(s.client_id.clone());
                                                    error!(
                                                        "Error send publilsh packet device broker {}",
                                                        err
                                                    );
                                                }
                                            }
                                            None => error!("Error: Not Exist sender"),
                                        }
                                    }
                                }
                            }
                        }
                    }
                    mqttcoder::MQTTPacket::Pingreq(_packet) => {
                        //debug!("Ping {:?}", _packet);
                        // ping / pingresp
                        let packet = mqttcoder::Pingresp::new();
                        if let Err(err) = tx.send(mqttcoder::MQTTPacket::Pingresp(packet)) {
                            return Err(io::Error::new(io::ErrorKind::Other, err));
                        };
                    }

                    _ => {}
                }
            }
            Err(err) => error!("error: {:?}", err),
        }
    }
    return Ok(());
}

async fn process_tls(
    socket: &mut TcpStream,
    acceptor: TlsAcceptor,
    config: Arc<Config>,
    sender: broadcast::Sender<PublishedPacket>,
    connection_map: connection_store::ConnectionStateDB,
    subscription_map: SubscriptionsDB,
    retain_store: RetainPacketDB,
) -> io::Result<()> {
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

    let (tx, rx) = mpsc::unbounded_channel();

    tokio::select! {
        _ = send_packet(frame_writer, rx) => {

        }
        _ = recv_packet(
            frame_reader,
            connection_map,
            tx,
            config,
            sender, subscription_map, retain_store) => {

            }
        /* [TODO] cancel from circuit breaker */
    }
    return Ok(());
}

async fn process_tcp(
    stream: &mut TcpStream,
    config: Arc<Config>,
    sender: broadcast::Sender<PublishedPacket>,
    connection_map: connection_store::ConnectionStateDB,
    subscription_map: SubscriptionsDB,
    retain_store: RetainPacketDB,
) -> io::Result<()> {
    let (rd, wr) = split(stream);

    let decoder = mqttcoder::MqttDecoder::new();
    let frame_reader = FramedRead::new(rd, decoder);
    let encoder = mqttcoder::MqttEncoder::new();

    let frame_writer = FramedWrite::new(wr, encoder);

    let (tx, rx) = mpsc::unbounded_channel();

    tokio::select! {
        _ = send_packet(frame_writer, rx) => {

        }
        _ = recv_packet(
            frame_reader,
            connection_map,
            tx,
            config,
            sender, subscription_map, retain_store) => {

            }
        /* [TODO] cancel from circuit breaker */
    }
    return Ok(());
}
