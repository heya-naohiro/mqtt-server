use crate::mqttcoder;
use std::sync::Arc;

use std::collections::HashMap;

use tokio::sync::mpsc;

pub type ConnectionStateDB = Arc<Mutex<HashMap<ConnectionKey, ConnectInfo>>>;
//pub type SubscriptionStateDB = Arc<Mutex<HashMap<Filter, SubscriptionInfo>>>;
type ConnectionKey = String; /* may be mqtt id or Common name */
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct Subfilter {
    pub filter: Filter,
    pub qos: u8,
}

#[derive(Debug)]
pub struct Filter {
    // all 8bit str
    pub elements: Vec<u8>,
}

#[derive(Debug)]
pub struct ConnectInfo {
    pub connect: mqttcoder::Connect,
    pub sender: mpsc::Sender<mqttcoder::MQTTPacket>,
    pub sub_filters: Vec<Subfilter>,
}

impl ConnectInfo {
    pub fn new(
        connect: mqttcoder::Connect,
        sender: mpsc::Sender<mqttcoder::MQTTPacket>,
    ) -> ConnectInfo {
        ConnectInfo {
            connect,
            sender,
            sub_filters: vec![],
            // [TODO] handover sub_filters if clean session is false ?
        }
    }
}
