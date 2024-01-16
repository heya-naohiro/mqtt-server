use bytes::Buf;
use bytes::BufMut;
use bytes::BytesMut;
use std::io::{Error, ErrorKind};
use std::str;
use tokio_util::codec::{Decoder, Encoder};
#[derive(Debug)]
pub enum MQTTPacket {
    Connect,
    Connack(Connack),
    Publish(Publish),
    Other,
}
pub enum MQTTPacketHeader {
    Connect,
    Connack,
    Publish,
    Other,
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

#[derive(Debug)]
pub struct Publish {
    topic_name: String,
    message_id: u32,
}

impl Publish {
    pub fn from_byte(buf: &mut BytesMut) -> Result<Option<(Publish, usize)>, Error> {
        // topic length : 2 byte + Message Identification length: 2byte
        if buf.len() < 4 {
            return Ok(None);
        }
        let topic_length: usize = ((buf[0] as usize) << 8) + buf[1] as usize;
        if buf.len() < topic_length + 4 {
            return Ok(None);
        }
        let slice = &buf[2..(2 + topic_length)];
        let topic_name = match std::str::from_utf8(slice) {
            Ok(v) => v,
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, "Invalid"));
            }
        };
        let message_id: u32 = ((buf[2 + topic_length] as u32) << 8) + buf[3 + topic_length] as u32;

        // [TODO]
        return Ok(Some((
            Publish {
                topic_name: String::from(topic_name),
                message_id,
            },
            4 + topic_length,
        )));
    }
}

pub struct MqttDecoder {
    header: Option<Header>,
}

impl MqttDecoder {
    pub fn new() -> MqttDecoder {
        MqttDecoder { header: None }
    }
}

struct Header {
    mtype: MQTTPacketHeader,
    dup: bool,
    qos: usize,
    retain: bool,
    remaining_length: usize,
}

impl Header {}

fn read_header(src: &mut BytesMut) -> Result<Option<(Header, usize)>, Error> {
    if src.len() < 2 {
        return Ok(None);
    } else {
        let byte = src[0];
        let mut advance = 1;
        let dup = byte & 0b00001000 == 0b00001000;
        let qos = (byte & 0b00000110) >> 1;
        let retain = byte & 0b00000001 == 0b00000110;
        let mut remaining_length: usize = 0;
        // "残りの長さ"の箇所は最大4つ
        for pos in 0..=3 {
            let byte = src[pos + 1];
            advance += 1;
            remaining_length += (byte as usize & 0b0111111) << (pos * 7);
            if (byte & 0b10000000) == 0 {
                break;
            } else {
                // check next byte
                if src.len() < pos + 2 {
                    // insufficient buffer size
                    return Ok(None);
                }
            }
        }
        let mtype = match byte >> 4 {
            1 => MQTTPacketHeader::Connect,
            3 => MQTTPacketHeader::Publish,
            _ => MQTTPacketHeader::Other,
        };
        return Ok(Some((
            Header {
                mtype,
                dup,
                qos: qos.into(),
                retain,
                remaining_length,
            },
            advance,
        )));
    }
}

impl Decoder for MqttDecoder {
    type Item = MQTTPacket;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match self.header {
            None => {
                let length = src.len();
                if src.len() < 2 {
                    return Ok(None);
                }

                // 固定ヘッダーに可変長(残りの長さを含むため固定ヘッダーを解読できたら読んだreadbyte分進める必要がある)
                let (header, readbyte) = match read_header(src) {
                    Ok(Some(value)) => value,
                    Ok(None) => return Ok(None),
                    Err(e) => return Err(e),
                };

                self.header = Some(header);
                src.advance(readbyte);
                match Some(self.header) {
                    MQTTPacketHeader::Connect => Ok(Some(MQTTPacket::Connect)),
                    MQTTPacketHeader::Publish => {
                        let (variable_header_only, readbyte) = match Publish::from_byte(src) {
                            Ok(Some(value)) => value,
                            Ok(None) => return Ok(None),
                            Err(e) => return Err(e),
                        };
                        src.advance(readbyte);
                        // TODO: add payload processing
                        Ok(Some(MQTTPacket::Publish(variable_header_only)))
                    }
                    _ => {
                        src.advance(length);
                        Err(Error::new(ErrorKind::Other, "Invalid"))
                    }
                }
            }
            Some(header) => match header.mtype {
                // [TODO] second packet implement
                _ => Err(Error::new(ErrorKind::Other, "Invalid")),
            },
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
