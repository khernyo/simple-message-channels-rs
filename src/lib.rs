mod decoder;
mod encoder;

pub use decoder::{Decoder, Listener};
pub use encoder::Encoder;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Channel(pub u64);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Type(pub u64);
