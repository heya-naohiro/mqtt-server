use bytes::{Buf, BytesMut};
use mqttrs::read_header;
use tokio_util::codec::Decoder;
pub struct MqttDecoder {}

const MAX: usize = 8 * 1024 * 1024;

impl Decoder for MqttDecoder {
    type Item = String;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            // not enoght data
            return Ok(None);
        }
        let mut length_bytes = [0u8; 4];
        length_bytes.copy_from_slice(&src[..4]);
        println!("hello: {:?}", src);
        let length = u32::from_le_bytes(length_bytes) as usize;
        if length > MAX {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", length),
            ));
        }
        if src.len() < 4 + length {
            src.reserve(4 + length - src.len());

            return Ok(None);
        }
        let data = src[4..4 + length].to_vec();
        src.advance(4 + length);

        // Convert the data to a string, or fail if it is not valid utf-8.
        match String::from_utf8(data) {
            Ok(string) => Ok(Some(string)),
            Err(utf8_error) => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                utf8_error.utf8_error(),
            )),
        }
    }
}

fn read_header<'a>(buf: &'a [u8], offset: &mut usize) -> Result<Option<(Header, usize)>, Error> {
    let mut len: usize = 0;
    for pos in 0..=3 {
        if buf.len() > *offset + pos + 1 {
            let byte = buf[*offset + pos + 1];
            len += (byte as usize & 0x7F) << (pos * 7);
            if (byte & 0x80) == 0 {
                // Continuation bit == 0, length is parsed
                if buf.len() < *offset + 2 + pos + len {
                    // Won't be able to read full packet
                    return Ok(None);
                }
                // Parse header byte, skip past the header, and return
                let header = Header::new(buf[*offset])?;
                *offset += pos + 2;
                return Ok(Some((header, len)));
            }
        } else {
            // Couldn't read full length
            return Ok(None);
        }
    }
    // Continuation byte == 1 four times, that's illegal.
    Err(Error::InvalidHeader)
}
