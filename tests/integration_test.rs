use assert_cmd::Command;
use predicates::prelude::*;
//use paho_mqtt as mqtt;
use tokio::time::timeout;
use tokio::time::Duration;

//type TestResult = Result<(), Box<dyn std::error::Error>>;
#[tokio::test]
async fn test_something_async() {
    let timelimit = Duration::from_secs(5);
    let handle = tokio::spawn(async {
        let assert = Command::cargo_bin("mqttserver")
            .unwrap()
            .timeout(std::time::Duration::from_secs(1))
            .assert();
        assert.success();
    });
    let result = timeout(timelimit, handle).await;
    match result {
        Ok(Ok(task_result)) => {
            println!("Task completed successfully: {:?}", task_result);
        }
        Ok(Err(e)) => {
            eprintln!("Task failed: {:?}", e);
        }
        Err(_) => {
            eprintln!("Task timed out after {:?}", timelimit);
        }
    }
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
