use std::io::Write;

use bytes::{BufMut, Bytes, BytesMut};
use integer_encoding::{VarInt, VarIntWriter};

use crate::{Channel, Type};

pub struct Encoder {
    max_size: usize,
}

impl Encoder {
    pub fn new<MS>(max_size: MS) -> Encoder
    where
        MS: Into<Option<usize>>,
    {
        let max_size = max_size.into().unwrap_or(8 * 1024 * 1024);
        Encoder { max_size }
    }

    pub fn send(&mut self, channel: Channel, r#type: Type, data: &Bytes) -> Result<Bytes, String> {
        if data.len() > self.max_size {
            return Err(format!(
                "Trying to encode message larger than max size: {} > {}",
                data.len(),
                self.max_size
            ));
        }

        let header = channel.0 << 4 | r#type.0;
        let length = data.len() + VarInt::required_space(header);

        let payload = BytesMut::with_capacity(VarInt::required_space(length) + length);

        let mut writer = payload.writer();
        writer.write_varint(length).unwrap();
        writer.write_varint(header).unwrap();
        writer.write_all(data).unwrap();

        Ok(writer.into_inner().freeze())
    }
}
