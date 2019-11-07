use bytes::Bytes;

mod decoder;
mod encoder;

pub use decoder::{Decoder, Event};
pub use encoder::Encoder;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Channel(pub u64);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Type(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub channel: Channel,
    pub r#type: Type,
    pub data: Bytes,
}
