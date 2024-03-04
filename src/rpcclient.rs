use rpcserver::published_packet_service_client::PublishedPacketServiceClient;
use rpcserver::PublishedPacketRequest;
use std::error::Error;
use tonic::transport::Channel;
use tonic::Request;

pub mod rpcserver {
    tonic::include_proto!("rpcserver");
}

pub async fn print_published_payloads(
    client: &mut PublishedPacketServiceClient<Channel>,
) -> Result<(), Box<dyn Error>> {
    let request = PublishedPacketRequest { device_id: None };
    let mut stream = client
        .stream_published_payload(Request::new(request))
        .await?
        .into_inner();
    while let Some(packet) = stream.message().await? {
        println!("Packet = {:?}", packet);
    }

    Ok(())
}
