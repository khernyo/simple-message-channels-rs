use bytes::{BufMut, Bytes, BytesMut};
use checked_int_cast::CheckedIntCast;

use crate::{Channel, Message, MessageType};

pub struct Decoder {
    destroyed: bool,
    error: Option<String>,

    message: Option<BytesMut>,
    varint: u64,
    factor: u64,
    length: u64,
    header: u64,
    state: State,
    consumed: u64,
    max_size: usize,
}

impl Decoder {
    pub fn new<MS>(max_size: MS) -> Decoder
    where
        MS: Into<Option<usize>>,
    {
        let max_size = max_size.into().unwrap_or(8 * 1024 * 1024);

        Decoder {
            destroyed: false,
            error: None,

            message: None,
            varint: 0,
            factor: 1,
            length: 0,
            header: 0,
            state: State::Length,
            consumed: 0,
            max_size,
        }
    }

    fn destroy(&mut self, err: Option<String>) {
        if err.is_some() {
            self.error = err;
        }
        self.destroyed = true;
    }

    pub fn messages(&mut self, data: Bytes) -> DecoderIterator {
        DecoderIterator {
            decoder: self,
            bytes: data,
        }
    }
}

#[derive(Eq, PartialEq)]
enum State {
    Length,
    Header,
    Payload,
}

pub struct DecoderIterator<'a> {
    decoder: &'a mut Decoder,
    bytes: Bytes,
}

impl<'a> Iterator for DecoderIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Self::Item> {
        self.recv()
    }
}

impl<'a> DecoderIterator<'a> {
    pub fn recv(&mut self) -> Option<Event> {
        assert!(!self.decoder.destroyed);

        while !self.bytes.is_empty() {
            let result = if self.decoder.state == State::Payload {
                self.read_message()
            } else {
                self.read_varint()
            };
            if result.is_some() {
                return result;
            }
        }

        let result = if self.decoder.state == State::Payload && self.decoder.length == 0 {
            self.read_message()
        } else {
            None
        };
        assert!(!self.decoder.destroyed);
        result
    }

    fn read_message(&mut self) -> Option<Event> {
        let msg_missing = self.decoder.length.as_usize_checked().unwrap();
        if self.bytes.len() >= msg_missing {
            let msg_missing_bytes = self.bytes.split_to(msg_missing);
            if let Some(ref mut msg) = self.decoder.message {
                msg.put_slice(&msg_missing_bytes);
            } else {
                self.decoder.message = Some(msg_missing_bytes.into());
            }
            return self.next_state();
        }

        if self.decoder.message.is_none() {
            self.decoder.message = Some(BytesMut::with_capacity(msg_missing));
        }
        let msg = self.decoder.message.as_mut().unwrap();
        msg.put_slice(&self.bytes);
        self.decoder.length -= self.bytes.len().as_u64_checked().unwrap();
        self.bytes.clear();
        None
    }

    fn read_varint(&mut self) -> Option<Event> {
        while !self.bytes.is_empty() {
            self.decoder.varint += (self.bytes[0] & 127) as u64 * self.decoder.factor;
            self.decoder.consumed += 1;
            if self.bytes[0] < 128 {
                self.bytes.advance(1);
                return self.next_state();
            }
            self.decoder.factor *= 128;
            self.bytes.advance(1);
        }
        if self.decoder.consumed >= 8 {
            self.decoder
                .destroy("Incoming varint is invalid".to_owned().into()); // 8 * 7bits is 56 ie max for js
            Some(Event::Error(Error::InvalidHeader))
        } else {
            None
        }
    }

    fn next_state(&mut self) -> Option<Event> {
        match self.decoder.state {
            State::Length => {
                self.decoder.state = State::Header;
                self.decoder.factor = 1;
                self.decoder.length = self.decoder.varint;
                self.decoder.varint = 0;
                self.decoder.consumed = 0;
                if self.decoder.length == 0 {
                    self.decoder.state = State::Length;
                }
                None
            }

            State::Header => {
                self.decoder.state = State::Payload;
                self.decoder.factor = 1;
                self.decoder.header = self.decoder.varint;
                self.decoder.length = self
                    .decoder
                    .length
                    .checked_sub(self.decoder.consumed)
                    .unwrap();
                self.decoder.varint = 0;
                self.decoder.consumed = 0;
                if self.decoder.length.as_usize_checked().unwrap() > self.decoder.max_size {
                    self.decoder
                        .destroy("Incoming message is larger than max size".to_owned().into());
                    return Some(Event::Error(Error::TooLarge(TooLarge {
                        channel: Channel(self.decoder.header >> 4),
                        message_type: MessageType(self.decoder.header & 0b1111),
                        len: self.decoder.length,
                    })));
                }

                let extra = self.bytes.len();
                let len = self.decoder.length.as_usize_checked().unwrap();
                if len > extra {
                    Some(Event::Missing(len - extra))
                } else {
                    None
                }
            }

            State::Payload => {
                assert!(!self.decoder.destroyed);
                self.decoder.state = State::Length;
                let msg = self.decoder.message.take().unwrap();
                Some(Event::Message(Message {
                    channel: Channel(self.decoder.header >> 4),
                    message_type: MessageType(self.decoder.header & 0b1111),
                    data: msg.freeze(),
                }))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    Message(Message),
    Missing(usize),
    Error(Error),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    TooLarge(TooLarge),
    InvalidHeader,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TooLarge {
    channel: Channel,
    message_type: MessageType,
    len: u64,
}
