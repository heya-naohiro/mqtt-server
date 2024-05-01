use crate::mqttcoder;
use std::sync::Arc;

use std::collections::HashMap;

use tokio::sync::mpsc;

pub type ConnectionStateDB = Arc<Mutex<HashMap<ConnectionKey, ConnectInfo>>>;
type ConnectionKey = String; /* may be mqtt id or Common name */
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct ConnectInfo {
    pub connect: mqttcoder::Connect,
    pub sender: mpsc::UnboundedSender<mqttcoder::MQTTPacket>,
}

impl ConnectInfo {
    pub fn new(
        connect: mqttcoder::Connect,
        sender: mpsc::UnboundedSender<mqttcoder::MQTTPacket>,
    ) -> ConnectInfo {
        ConnectInfo {
            connect,
            sender,
            // [TODO] handover sub_filters if clean session is false ?
        }
    }
}
