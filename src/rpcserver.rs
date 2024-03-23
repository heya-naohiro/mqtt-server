use crate::connection_store;
use crate::mqttcoder::Publish;
use crate::rpcserver::rpcserver::published_packet_service_server::PublishedPacketService;
use crate::MQTTPacket;

use rpcserver::{PublishedPacket};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::Mutex as TokioMutex;
use tokio_stream::wrappers::ReceiverStream;

use tonic::{Request, Response, Status};

pub mod rpcserver {
    tonic::include_proto!("rpcserver");
}

#[derive(Debug)]
pub struct PlatformPublishedPacketService {
    pub reciever: Arc<TokioMutex<broadcast::Receiver<PublishedPacket>>>,
    pub connection_map: connection_store::ConnectionStateDB,
}

impl PlatformPublishedPacketService {
    // コンストラクタを追加
    pub fn new(
        receiver: broadcast::Receiver<PublishedPacket>,
        connection_map: connection_store::ConnectionStateDB,
    ) -> Self {
        let arc_receiver = Arc::new(TokioMutex::new(receiver));
        Self {
            reciever: arc_receiver,
            connection_map,
        }
    }
}

#[tonic::async_trait]
impl PublishedPacketService for PlatformPublishedPacketService {
    type StreamPublishedPayloadStream = ReceiverStream<Result<PublishedPacket, Status>>;
    //  expected enum `std::result::Result<tonic::Response<ReceiverStream<std::result::Result<PublishedPacket, Status>>>, Status>`

    /* Device -> Server -> gRPCクライアントにStream */
    async fn stream_published_payload(
        &self,
        request: Request<rpcserver::PublishedPacketRequest>,
    ) -> Result<Response<Self::StreamPublishedPayloadStream>, Status> {
        println!("Request PublishedPacket = {:?}", request);

        let (tx, rx) = mpsc::channel(4);
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

    /* gRPCクライアント -> Server -> Device に 1 packet Push */
    async fn publish_payload_to_device(
        &self,
        request: Request<rpcserver::PublishRequest>,
    ) -> Result<Response<rpcserver::PublishResponse>, Status> {
        println!("Publish Request {:?}", request);
        let cmap = self.connection_map.lock().await;

        let req = request.into_inner().to_owned();
        let device_id = req.device_id;
        let payload = req.payload;
        let send_mqtt_packet = Publish::new(device_id.clone(), 0 as u32, payload);
        {
            let info: Option<&connection_store::ConnectInfo> = cmap.get(&device_id);

            match info {
                Some(i) => {
                    if let Err(err) = i.sender.send(MQTTPacket::Publish(send_mqtt_packet)).await {
                        println!("Publish error {:?}", err);
                    }
                }
                None => {
                    println!("Not Found sender");
                }
            }
        }


        let reply = rpcserver::PublishResponse {
            code: "Success".into(),
        };
        Ok(Response::new(reply))
    }
}
