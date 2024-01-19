use bytes::Buf;
use bytes::BufMut;
use bytes::BytesMut;
use mqttrs::Packet;
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
#[derive(Debug)]
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

#[derive(Debug, Clone)] // Cloneを追加
pub struct Publish {
    topic_name: String,
    message_id: u32,
    // まずは小さいサイズ想定ですべてVec<u8>にコピーする
    payload: Vec<u8>,
}

impl Publish {
    pub fn payload_from_byte(&mut self, buf: &mut BytesMut, remain: usize) -> Result<usize, Error> {
        println!("->>>> {:?}", buf);
        if buf.len() > remain {
            let added_vec: Vec<u8> = buf[..remain].to_vec();
            self.payload.extend_from_slice(&added_vec);
            return Ok(added_vec.len());
        }
        let added_vec: Vec<u8> = buf.to_vec();
        self.payload.extend_from_slice(&added_vec);
        return Ok(added_vec.len());
    }

    pub fn from_byte(buf: &mut BytesMut, qos0: bool) -> Result<Option<(Publish, usize)>, Error> {
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
            Err(_) => {
                return Err(Error::new(ErrorKind::Other, "Invalid"));
            }
        };
        let (message_id, readsize) = if qos0 {
            (0, 2 + topic_length)
        } else {
            (
                ((buf[2 + topic_length] as u32) << 8) + buf[3 + topic_length] as u32,
                4 + topic_length,
            )
        };

        //let message_id: u32 =

        // [TODO]
        return Ok(Some((
            Publish {
                topic_name: String::from(topic_name),
                message_id,
                payload: vec![],
            },
            readsize,
        )));
    }
}

pub struct MqttDecoder {
    header: Option<Header>,
    packet: Option<MQTTPacket>,
    realremaining_length: usize,
}

impl MqttDecoder {
    pub fn new() -> MqttDecoder {
        MqttDecoder {
            header: None,
            packet: None,
            realremaining_length: 0,
        }
    }
}
#[derive(Debug)]
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
        match &self.header {
            None => {
                let length = src.len();
                println!("Length: {:?}", length);
                if src.len() < 2 {
                    return Ok(None);
                }

                // 固定ヘッダーに可変長(残りの長さを含むため固定ヘッダーを解読できたら読んだreadbyte分進める必要がある)
                let (header, readbyte) = match read_header(src) {
                    Ok(Some(value)) => value,
                    Ok(None) => return Ok(None),
                    Err(e) => return Err(e),
                };
                self.realremaining_length = header.remaining_length;
                // 後にheader.mtypeでパターンマッチするのでここでselfに格納しない
                //self.header = Some(header);
                println!("header {:?}", header);
                println!("fixed header advance {:?} bytes", readbyte);

                src.advance(readbyte);
                match header.mtype {
                    MQTTPacketHeader::Connect => {
                        //これ以上処理しないので（いまのところ）残りのbyteを破棄する
                        src.advance(src.len());
                        Ok(Some(MQTTPacket::Connect))
                    }
                    MQTTPacketHeader::Publish => {
                        // Decoderに格納する
                        //let remain_length = header.remaining_length;
                        //
                        let (variable_header_only, readbyte) =
                            match Publish::from_byte(src, header.qos == 0) {
                                Ok(Some(value)) => value,
                                Ok(None) => return Ok(None),
                                Err(e) => return Err(e),
                            };

                        println!(
                            "variable header advance {:?} bytes, realremaining_length {:?}",
                            readbyte, self.realremaining_length
                        );
                        println!("variable header {:?}", variable_header_only);
                        src.advance(readbyte);
                        // save packet temporary
                        println!("byte check {:?} {:?}", self.realremaining_length, readbyte);
                        if self.realremaining_length < readbyte {
                            return Err(Error::new(ErrorKind::Other, "Invalid byte size zbbb"));
                        }
                        self.realremaining_length = self.realremaining_length - readbyte;

                        self.packet = Some(MQTTPacket::Publish(variable_header_only));
                        self.header = Some(header);
                        // process publish packet
                        // 強制的に次のターンに持ち込みpaylodを処理する（残りが何byteであろうと)
                        println!("next!!!");
                        Ok(None)
                    }
                    _ => {
                        //これ以上処理しないので（いまのところ）残りのbyteを破棄する
                        src.advance(src.len());
                        Err(Error::new(ErrorKind::Other, "Invalid"))
                    }
                }
            }
            // ここに来るということは、variable headerも読んだ状態、つまりpayloadの処理
            Some(header) => match header.mtype {
                // [TODO] second packet implement
                /*
                _ => {
                    let packet = self.packet.take();
                    return Ok(packet);
                } */
                //
                MQTTPacketHeader::Publish => match self.packet.take() {
                    Some(MQTTPacket::Publish(mut publish)) => {
                        println!("HERE!!!!!111111");
                        let readbyte =
                            match publish.payload_from_byte(src, self.realremaining_length) {
                                Ok(value) => value,
                                Err(_error) => {
                                    return Err(Error::new(ErrorKind::Other, "Invalid"));
                                }
                            };
                        src.advance(readbyte);
                        println!("HERE!!!!! {:?}, {:?}", self.realremaining_length, readbyte);

                        if self.realremaining_length < readbyte {
                            return Err(Error::new(ErrorKind::Other, "Invalid byte size AAA"));
                        }

                        self.realremaining_length = self.realremaining_length - readbyte;
                        if self.realremaining_length > src.len() {
                            Ok(None)
                        } else {
                            {
                                let strpayload_check =
                                    String::from_utf8(publish.payload.clone()).unwrap();
                                println!("Packet publish {:?}", strpayload_check);
                            }
                            Ok(Some(MQTTPacket::Publish(publish.clone())))
                        }
                    }
                    _ => {
                        println!("Error arienai");
                        Err(Error::new(ErrorKind::Other, "Invalid"))
                    }
                },
                _ => {
                    println!("Second packet not implement {:?}", header.mtype);
                    Err(Error::new(ErrorKind::Other, "Invalid"))
                }
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
