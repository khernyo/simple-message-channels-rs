use bytes::Bytes;

mod decoder;
mod encoder;

pub use decoder::{Decoder, Event};
pub use encoder::Encoder;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Channel(pub u64);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MessageType(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub channel: Channel,
    pub message_type: MessageType,
    pub data: Bytes,
}
