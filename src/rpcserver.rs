use crate::rpcserver::rpcserver::published_packet_service_server::PublishedPacketService;
use async_channel::Receiver;
use rpcserver::{PublishedPacket, PublishedPacketRequest};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, Stream, StreamExt};
use tonic::{Request, Response, Status};

pub mod rpcserver {
    tonic::include_proto!("rpcserver");
}

#[derive(Debug, Clone)]
pub struct PlatformPublishedPacketService {
    pub packet_channel: Arc<Receiver<PublishedPacket>>,
}

#[tonic::async_trait]
impl PublishedPacketService for PlatformPublishedPacketService {
    type StreamPublishedPayloadStream = ReceiverStream<Result<PublishedPacket, Status>>;
    //  expected enum `std::result::Result<tonic::Response<ReceiverStream<std::result::Result<PublishedPacket, Status>>>, Status>`

    async fn stream_published_payload(
        &self,
        request: Request<rpcserver::PublishedPacketRequest>,
    ) -> Result<Response<Self::StreamPublishedPayloadStream>, Status> {
        println!("Request PublishedPacket = {:?}", request);
        // [TODO]4から適切な値に変更する？
        let (tx, rx) = mpsc::channel(4); //この型は
        let channel = self.packet_channel.clone();
        tokio::spawn(async move {
            loop {
                let packet = channel.recv().await.unwrap(); //ここから型推論される
                tx.send(Ok(packet)).await.unwrap();
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
