use bytes::Buf;
use bytes::BufMut;
use bytes::BytesMut;
use std::io::{Error, ErrorKind};
use tokio_util::codec::{Decoder, Encoder};
#[derive(Debug)]
pub enum MQTTPacket {
    Connect(Connect),
    Connack(Connack),
    Publish(Publish),
    Disconnect,
    Subscribe(Subscribe),
    Suback(Suback),
    _Other,
}
#[derive(Debug)]
pub enum MQTTPacketHeader {
    Connect,
    Disconnect,
    _Connack,
    Publish,
    Subscribe,
    Other,
}

/* Procol Version */
#[derive(Debug)]
pub enum ProtocolVersion {
    V3,
    V3_1,
    V3_1_1,
    V5,
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
pub struct Suback {
    message_id: u16,
    sublength: usize,
}
impl Suback {
    // [TODO] all qos 0 now
    pub fn new(message_id: u16, sublength: usize) -> Suback {
        // [TODO] Implement actual operation and return code
        Suback {
            message_id,
            sublength,
        }
    }

    pub fn to_buf(&self, buf: &mut BytesMut) {
        let header: u8 = 0b10010000;
        let remain_length: usize = 2 /* id */+ self.sublength /* qos 1byte * sublen */; // [TODO] multi byte

        buf.put_u8(header);
        buf.put_u8(remain_length as u8);
        buf.put_u8((self.message_id >> 8) as u8);
        buf.put_u8(self.message_id as u8);
        for _ in 1..=self.sublength {
            // all qos 0 ([TODO] qos negotiation)
            buf.put_u8(0);
        }
    }
}

/*  Request Connection */
#[derive(Debug)]
pub struct Connect {
    pub protocol_ver: ProtocolVersion,
    pub clean_session: bool,
    pub will: bool,
    pub will_qos: u8,
    pub will_retain: bool,
    pub user_password_flag: bool,
    pub user_name_flag: bool,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
}
impl Connect {
    // variable header
    // 長さチェック済み
    pub fn from_byte(buf: &mut BytesMut) -> Result<Option<(Connect, usize)>, Error> {
        let protocol_name_length = ((buf[0] as usize) << 8) + buf[1] as usize;
        println!("protocol_name_length: {:?}", protocol_name_length);

        // e.g. MQIsdp v3.1
        let protocolname = if let Ok(str) = std::str::from_utf8(&buf[2..2 + protocol_name_length]) {
            str.to_owned()
        } else {
            return Err(Error::new(ErrorKind::Other, "Invalid"));
        };

        println!("protocolname: {:?}", protocolname);
        /* [TODO] protocol name check, if invalid close the connection anyway */

        let offset = 2 + protocol_name_length;
        let protocol_version = buf[offset];
        println!("version: {:?}", protocol_version);

        // Connection Flag bit 1
        let clean_session: bool = ((buf[offset + 1] & 0b00000010) >> 1) == 0b00000001;
        // Connection Flag bit 2
        let will: bool = ((buf[offset + 1] & 0b00000100) >> 2) == 0b00000001;
        let will_qos: u8 = (buf[offset + 1] & 0b00011000) >> 3;
        let will_retain: bool = ((buf[offset + 1] & 0b00100000) >> 5) == 0b00000001;
        let user_password_flag: bool = ((buf[offset + 1] & 0b01000000) >> 6) == 0b00000001;
        let user_name_flag: bool = ((buf[offset + 1] & 0b10000000) >> 7) == 0b00000001;
        // big endian = 上位ビットが先
        let keepalive_timer: u16 = ((buf[offset + 2] as u16) << 8) + buf[offset + 3] as u16;
        println!("keepalive_timer: {:?}", keepalive_timer);

        // Client Identification , Must have
        let client_id_length = ((buf[offset + 4] as usize) << 8) + buf[offset + 5] as usize;
        let client_id = if let Ok(str) =
            std::str::from_utf8(&buf[(offset + 6)..(offset + 6 + client_id_length)])
        {
            str.to_owned()
        } else {
            return Err(Error::new(ErrorKind::Other, "Invalid"));
        };
        let mut offset = offset + 6 + client_id_length;
        // Will: [TODO] Not implemented
        if will {
            let will_topic_length = ((buf[offset] as usize) << 8) + buf[offset + 2] as usize;
            offset = 2 + will_topic_length;
            let will_message_length = ((buf[offset] as usize) << 8) + buf[offset + 2] as usize;
            // todo will packet struct
            offset = 2 + will_message_length;
        }

        // Username
        let mut username = None;
        if user_name_flag {
            let username_length = ((buf[offset] as usize) << 8) + buf[offset + 2] as usize;
            username =
                if let Ok(str) = std::str::from_utf8(&buf[(offset)..(offset + username_length)]) {
                    Some(str.to_owned())
                } else {
                    return Err(Error::new(ErrorKind::Other, "Invalid"));
                };
            offset = 2 + username_length;
        }

        // Password
        let mut password = None;
        if user_password_flag {
            let password_length = ((buf[offset] as usize) << 8) + buf[offset + 2] as usize;
            password =
                if let Ok(str) = std::str::from_utf8(&buf[(offset)..(offset + password_length)]) {
                    Some(str.to_owned())
                } else {
                    return Err(Error::new(ErrorKind::Other, "Invalid"));
                };
            offset = 2 + password_length;
        }
        // Password

        let protocol_ver = ProtocolVersion::V3_1;
        Ok(Some((
            Connect {
                protocol_ver,
                clean_session,
                will,
                will_qos,
                will_retain,
                user_name_flag,
                user_password_flag,
                username,
                password,
                client_id,
            },
            offset,
        )))
    }
}

#[derive(Debug)]
pub struct Subscribe {
    pub message_id: u16,
    pub subscription_list: Vec<SubscriptionInfo>,
}

#[derive(Debug)]
pub struct SubscriptionInfo {
    topicfilter: String,
    qos: u8,
}

impl Subscribe {
    pub fn from_byte(buf: &BytesMut) -> Result<Option<(Subscribe, usize)>, Error> {
        if buf.len() < 2 {
            return Ok(None);
        }
        println!("buf[1]: {:?} buf[0] {:?}", buf[1], buf[0]);
        let message_id: u16 = (buf[1] as u16) + ((buf[0] as u16) << 8);
        println!("message id === {:?}", message_id);
        Ok(Some((
            Subscribe {
                message_id,
                subscription_list: vec![],
            },
            2,
        )))
    }
    pub fn payload_from_byte(&mut self, buf: &mut BytesMut, remain: usize) -> Result<usize, Error> {
        // todo remain
        let mut sub_counter: usize = 0;
        loop {
            let topiclength: usize =
                buf[sub_counter + 1] as usize + ((buf[sub_counter] as usize) << 8);
            sub_counter = sub_counter + 2;
            let topicfilter = if let Ok(str) =
                std::str::from_utf8(&buf[sub_counter..sub_counter + topiclength])
            {
                str.to_owned()
            } else {
                return Err(Error::new(ErrorKind::Other, "Invalid"));
            };
            sub_counter = sub_counter + topiclength;
            println!("topic fileter {:?}", topicfilter);

            let qos = match buf[sub_counter] {
                0 => 0,
                1 => 1,
                2 => 2,
                value @ _ => {
                    println!("qos {:?}", value);
                    return Err(Error::new(ErrorKind::Other, "Invalid qos "));
                }
            };
            sub_counter = sub_counter + 1;
            self.subscription_list
                .push(SubscriptionInfo { topicfilter, qos });

            // 異常系
            if remain <= sub_counter {
                break;
            }
        }

        return Ok(sub_counter);
    }
}

#[derive(Debug, Clone)] // Cloneを追加
pub struct Publish {
    pub topic_name: String,
    pub message_id: u32,
    // まずは小さいサイズ想定ですべてVec<u8>にコピーする
    pub payload: Vec<u8>,
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
    pub fn reset(&mut self) {
        self.header = None;
        self.packet = None;
        self.realremaining_length = 0;
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
            14 => MQTTPacketHeader::Disconnect,
            3 => MQTTPacketHeader::Publish,
            8 => MQTTPacketHeader::Subscribe,
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
        println!("Decode top header ??? {:?}", self.header);
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
                        let result = Connect::from_byte(src);
                        let (packet, size) = match result {
                            Ok(Some((packet, size))) => (packet, size),
                            Ok(None) => {
                                // TODO
                                return Ok(None);
                            }
                            Err(err) => {
                                self.reset();
                                return Err(err);
                            }
                        };
                        println!("size: {}, length: {}", size, src.len());
                        // [TODO] advanceはheaderベースでやって安全性を高めること下記のように
                        src.advance(self.realremaining_length);
                        self.reset();
                        Ok(Some(MQTTPacket::Connect(packet)))
                    }
                    MQTTPacketHeader::Disconnect => {
                        src.advance(src.len());
                        self.reset();
                        Ok(Some(MQTTPacket::Disconnect))
                    }
                    MQTTPacketHeader::Subscribe => {
                        let (mut variable_header_only, readbyte) = match Subscribe::from_byte(src) {
                            Ok(Some(value)) => value,
                            Ok(None) => return Ok(None),
                            Err(e) => return Err(e),
                        };
                        // [TODO] advanceはheaderベースでやって安全性を高める
                        src.advance(readbyte);
                        if self.realremaining_length < readbyte {
                            return Err(Error::new(ErrorKind::Other, "Invalid byte size zbbb"));
                        }
                        self.realremaining_length = self.realremaining_length - readbyte;

                        let readbyte = match variable_header_only
                            .payload_from_byte(src, self.realremaining_length)
                        {
                            Ok(value) => value,
                            Err(e) => return Err(e),
                        };
                        // [TODO] advanceはheaderベースでやって安全性を高める
                        src.advance(readbyte);
                        self.reset();
                        Ok(Some(MQTTPacket::Subscribe(variable_header_only)))
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
                        // [TODO] advanceはheaderベースでやって安全性を高める
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
                        // 続きを処理しなければならない
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
                MQTTPacketHeader::Subscribe => match self.packet.take() {
                    Some(MQTTPacket::Subscribe(mut subscribe)) => {
                        println!("Here ?????????");
                        let readbyte =
                            match subscribe.payload_from_byte(src, self.realremaining_length) {
                                Ok(value) => value,
                                Err(_error) => {
                                    return Err(Error::new(ErrorKind::Other, "Invalid"));
                                }
                            };
                        if readbyte == 0 {
                            return Ok(None);
                        }
                        src.advance(readbyte);
                        self.realremaining_length = self.realremaining_length - readbyte;
                        if self.realremaining_length > src.len() {
                            Ok(None)
                        } else {
                            println!("Packet publish {:?}", subscribe.subscription_list);

                            // reset for next
                            self.reset();
                            Ok(Some(MQTTPacket::Subscribe(subscribe)))
                        }
                    }
                    _ => {
                        println!("subscribe Second packet not implement {:?}", header.mtype);
                        Err(Error::new(ErrorKind::Other, "Invalid"))
                    }
                },
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
                            // reset for next
                            self.reset();
                            Ok(Some(MQTTPacket::Publish(publish)))
                        }
                    }
                    _ => {
                        println!("Error arienai, {:?}, {:x?}", src.len(), src);
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
            MQTTPacket::Suback(x) => x.to_buf(buf),
            _ => {}
        }
        return Ok(());
    }
}
