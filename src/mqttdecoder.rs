use bytes::BytesMut;
use std::io::{Error, ErrorKind};
use tokio_util::codec::Decoder;

#[derive(Debug)]
pub enum MQTTPacket {
    Connect,
}

pub struct MqttDecoder {
    header: bool,
}

impl MqttDecoder {
    pub fn new() -> MqttDecoder {
        MqttDecoder { header: true }
    }
}

impl Decoder for MqttDecoder {
    type Item = MQTTPacket;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if self.header {
            if src.len() < 2 {
                return Ok(None);
            }
            let byte = src[0];

            match byte >> 4 {
                1 => Ok(Some(MQTTPacket::Connect)),
                _ => Err(Error::new(ErrorKind::Other, "Invalid")),
            }
        } else {
            Err(Error::new(ErrorKind::Other, "Not Implemented"))
        }
    }
}
