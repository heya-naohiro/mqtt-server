use futures::executor::block_on;
use paho_mqtt as mqtt;

use mqttserver;

#[test]
fn it_adds_two() {
    assert_eq!(4, mqttserver::add_two(2));
}

#[tokio::test]
async fn test_something_async() {
    //main().await;
    println!("tokio test aaaaaaaa");

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
}
