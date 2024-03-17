use rand::distributions::{Alphanumeric, DistString};
use rpcserver::published_packet_service_client::PublishedPacketServiceClient;
use rpcserver::{PublishRequest, PublishedPacketRequest};
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

pub async fn publish_random_payloads(
    client: &mut PublishedPacketServiceClient<Channel>,
) -> Result<(), Box<dyn Error>> {
    //let mut rng = rand::thread_rng();
    //let random_code = Alphanumeric.sample_string(&mut rng, 32);
    let payload: String = String::from("hello payload world😀"); // + &random_code;
                                                                 //let payload = format!("{}{}", payload, random_code);
    let request = PublishRequest {
        device_id: "test_device".to_string(),
        topic: "hello/world".to_string(),
        payload: payload.into_bytes(),
    };
    if let Err(err) = client.publish_payload_to_device(request).await {
        println!("Publish Error {:?}", err)
    };

    Ok(())
}
