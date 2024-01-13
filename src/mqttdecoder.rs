use bytes::{Buf, BytesMut};
use mqttrs::{self, Packet};
use tokio_util::codec::Decoder;
pub struct MqttDecoder {}

const MAX: usize = 8 * 1024 * 1024;

impl Decoder for MqttDecoder {
    type Item = String;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match mqttrs::decode_slice(src) {
            Ok(packet) => {
                println!("Some {:?}", packet);
                return Ok(Some("HEllo".to_string()));
            }
            _ => {
                return Ok(Some("Error".to_string()));
            }
        }
    }
}
