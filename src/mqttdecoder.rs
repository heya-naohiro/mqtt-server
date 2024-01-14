use bytes::Buf;
use bytes::BufMut;
use bytes::BytesMut;
use std::io::{Error, ErrorKind};
use tokio_util::codec::{Decoder, Encoder};
#[derive(Debug)]
pub enum MQTTPacket {
    Connect,
    Connack(Connack),
}

#[derive(Debug)]
pub struct Connack {
    session_present: bool,
    return_code: u8,
}
impl Connack {
    pub fn new() -> Connack {
        // [TODO] Implement actual operation and return code
        Connack {
            session_present: false,
            return_code: 0,
        }
    }

    pub fn to_buf(&self, buf: &mut BytesMut) {
        let header: u8 = 0b00100000;
        let length: u8 = 2;
        let mut flags: u8 = 0b00000000;
        if self.session_present {
            flags |= 0b1;
        }
        buf.put_u8(header);
        buf.put_u8(length);
        buf.put_u8(flags);
        buf.put_u8(self.return_code);
    }
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
            let length = src.len();
            if src.len() < 2 {
                return Ok(None);
            }
            let byte = src[0];

            match byte >> 4 {
                1 => {
                    src.advance(length);
                    Ok(Some(MQTTPacket::Connect))
                }
                _ => {
                    src.advance(length);
                    Err(Error::new(ErrorKind::Other, "Invalid"))
                }
            }
        } else {
            Err(Error::new(ErrorKind::Other, "Not Implemented"))
        }
    }
}

pub struct MqttEncoder {}

impl MqttEncoder {
    pub fn new() -> MqttEncoder {
        MqttEncoder {}
    }
}

impl Encoder<MQTTPacket> for MqttEncoder {
    type Error = std::io::Error;

    fn encode(&mut self, packet: MQTTPacket, buf: &mut BytesMut) -> Result<(), Self::Error> {
        match packet {
            MQTTPacket::Connack(x) => x.to_buf(buf),
            _ => {}
        }
        return Ok(());
    }
}
