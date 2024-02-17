use paho_mqtt as mqtt;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_connect_and_publish() {
    let certs = mqttserver::load_certs(Path::new("server.crt")).unwrap();
    let key = mqttserver::load_keys(Path::new("private.key")).unwrap();
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))
        .unwrap();
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 7878);

    let config = mqttserver::Config {
        serverconfig: config,
        address: addr,
    };
    let (sender, receiver) = mpsc::channel::<bool>(1);
    let task = tokio::spawn(mqttserver::run_main(config, receiver));

    let cli = mqtt::CreateOptionsBuilder::new()
        .server_uri("ssl://localhost:7878")
        .client_id("test_client_id")
        .max_buffered_messages(100)
        .create_client()
        .unwrap();
    let ssl_opts = mqtt::SslOptionsBuilder::new().verify(false).finalize();
    let conn_opts = mqtt::ConnectOptionsBuilder::new()
        .ssl_options(ssl_opts)
        .finalize();

    let ret = cli.connect(conn_opts).await;
    assert!(
        ret.is_ok(),
        "Expected Connect Result to be Ok, but got Err: {:?}",
        ret.err()
    );

    let msg = mqtt::MessageBuilder::new()
        .topic("test")
        .payload("Hello ssl mqtt world!")
        .qos(0)
        .finalize();
    let ret = cli.publish(msg).await;
    assert!(
        ret.is_ok(),
        "Expected Publish Result to be Ok, but got Err: {:?}",
        ret.err()
    );
    let ret = cli.disconnect(None).await;
    assert!(
        ret.is_ok(),
        "Expected Disconnect Result to be Ok, but got Err: {:?}",
        ret.err()
    );

    // stop mqtt
    let _ = sender.send(true).await;
    let _ = task.await.expect("server panicked");
}
