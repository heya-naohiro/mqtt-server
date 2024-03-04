use rpcclient::rpcserver::published_packet_service_client::PublishedPacketServiceClient;

mod rpcclient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = PublishedPacketServiceClient::connect("http://[::1]:10000").await?;
    println!("\n*** SERVER STREAMING ***");
    rpcclient::print_published_payloads(&mut client).await?;
    Ok(())
}
