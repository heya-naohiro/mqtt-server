use crate::rpcserver::rpcserver::published_packet_service_server::PublishedPacketService;
use rpcserver::{PublishedPacket};
use std::sync::{Arc};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::Mutex as TokioMutex;
use tokio_stream::{wrappers::ReceiverStream};
use tonic::{Request, Response, Status};

pub mod rpcserver {
    tonic::include_proto!("rpcserver");
}

#[derive(Debug)]
pub struct PlatformPublishedPacketService {
    pub reciever: Arc<TokioMutex<broadcast::Receiver<PublishedPacket>>>,
}

impl PlatformPublishedPacketService {
    // コンストラクタを追加
    pub fn new(receiver: broadcast::Receiver<PublishedPacket>) -> Self {
        let arc_receiver = Arc::new(TokioMutex::new(receiver));
        Self {
            reciever: arc_receiver,
        }
    }
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

        let (tx, rx) = mpsc::channel(4); //この型は
        let receiver_clone = Arc::clone(&self.reciever);
        tokio::spawn(async move {
            println!("RPC Service Daemon Start");

            loop {
                let packet = receiver_clone.lock().await.recv().await.unwrap();
                println!("Daemon {:?}", packet);
                tx.send(Ok(packet)).await.unwrap();
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
