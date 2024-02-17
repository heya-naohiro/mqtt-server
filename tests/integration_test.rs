use assert_cmd::Command;
use predicates::prelude::*;
use tokio::main;
use tokio::sync::mpsc;
//use paho_mqtt as mqtt;
use std::io::{self, BufReader};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use tokio::time::{sleep, Duration};

//type TestResult = Result<(), Box<dyn std::error::Error>>;
#[tokio::test]
async fn test_something_async() {
    //https://github.com/tokio-rs/axum/blob/d703e6f97a0156177466b6741be0beac0c83d8c7/examples/testing-websockets/src/main.rs#L103
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

    sleep(Duration::from_secs(10)).await;
    let _ = sender.send(true).await;

    task.await.expect("server panicked");

    /*
    if let Err(err) = block_on(async {
        let cli = mqtt::CreateOptionsBuilder::new()
            .server_uri("ssl://localhost:7878")
            .client_id("test_client_id")
            .max_buffered_messages(100)
            .create_client()?;
        let ssl_opts = mqtt::SslOptionsBuilder::new().verify(false).finalize();
        let conn_opts = mqtt::ConnectOptionsBuilder::new()
            .ssl_options(ssl_opts)
            .user_name("testuser")
            .password("testpassword")
            .finalize();

        cli.connect(conn_opts).await?;

        let msg = mqtt::MessageBuilder::new()
            .topic("test")
            .payload("Hello ssl mqtt world!")
            .qos(0)
            .finalize();
        cli.publish(msg).await?;
        cli.disconnect(None).await?;

        Ok::<(), mqtt::Error>(())
    }) {
        eprintln!("{}", err);
    }
    */
}

/*
#[tokio::test]
async fn test_connect_tls() {
    let handle = start_server();
    let create_opts = mqtt::CreateOptionsBuilder::new()
        .server_uri("ssl://localhost:7878")
        .client_id("rust_publish")
        .mqtt_version(MQTT_VERSION_3_1)
        .finalize();

    let cli = mqtt::Client::new(create_opts).unwrap_or_else(|err| {
        panic!("Error: {}", err);
        // デフォルト値や別の処理を行うこともできる
    });

    let conn_opts = mqtt::ConnectOptionsBuilder::new()
        .ssl_options(SslOptions::default())
        .clean_session(true)
        .finalize();

    let ret = cli.connect(conn_opts);

    // assert! マクロを使用して Result が Ok であることを検証
    assert!(
        ret.is_ok(),
        "Expected Result to be Ok, but got Err: {:?}",
        ret.err()
    );
    handle.abort()
}

*/
