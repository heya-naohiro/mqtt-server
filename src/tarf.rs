use rpcclient::rpcserver::published_packet_service_client::PublishedPacketServiceClient;
use tonic::transport::Channel;

mod rpcclient;

async fn hello_world(
    mut client: PublishedPacketServiceClient<Channel>,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        rpcclient::publish_random_payloads(&mut client).await?;
    }
    //return Ok(());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = PublishedPacketServiceClient::connect("http://[::1]:10000").await?;
    println!("\n*** SERVER STREAMING ***");
    let client2 = client.clone();
    tokio::spawn(async move {
        if let Err(err) = hello_world(client2).await {
            println!("Error, Hello world {:?}", err);
            return;
        }
    });
    rpcclient::print_published_payloads(&mut client).await?;

    Ok(())
}
