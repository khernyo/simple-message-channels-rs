use bytes::Bytes;
use checked_int_cast::CheckedIntCast;

mod decoder;
mod encoder;

pub use decoder::{Decoder, Event};
pub use encoder::Encoder;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Channel(pub u8);

impl Channel {
    const MAX_CHANNELS: u64 = 128;
}

impl From<u64> for Channel {
    fn from(value: u64) -> Self {
        assert!(value < Channel::MAX_CHANNELS, "{}", value);
        Channel(value as u8)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MessageType(pub u8);

impl MessageType {
    const MAX_MESSAGE_TYPE: u64 = 0x10;
}

impl From<u64> for MessageType {
    fn from(value: u64) -> Self {
        assert!(value < MessageType::MAX_MESSAGE_TYPE, "{}", value);
        MessageType(value.as_u8_checked().unwrap())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub channel: Channel,
    pub message_type: MessageType,
    pub data: Bytes,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Header {
    channel: Channel,
    message_type: MessageType,
}

impl From<u64> for Header {
    fn from(n: u64) -> Self {
        let message_type = MessageType::from(n & 0x0f);
        let channel = Channel::from(n >> 4);
        Header {
            channel,
            message_type,
        }
    }
}

fn encode_header(header: Header) -> u16 {
    assert!((header.channel.0 as u64) < Channel::MAX_CHANNELS);
    assert!((header.message_type.0 as u64) < MessageType::MAX_MESSAGE_TYPE);
    u16::from(header.channel.0) << 4 | header.message_type.0 as u16
}
