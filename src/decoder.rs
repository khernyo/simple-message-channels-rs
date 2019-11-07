use bytes::{BufMut, Bytes, BytesMut};
use checked_int_cast::CheckedIntCast;

use crate::{Channel, Type};

pub struct Decoder<L> {
    listener: L,

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

impl<L: Listener> Decoder<L> {
    pub fn new<MS>(max_size: MS, listener: L) -> Decoder<L>
    where
        MS: Into<Option<usize>>,
    {
        let max_size = max_size.into().unwrap_or(8 * 1024 * 1024);

        Decoder {
            listener,

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

    pub fn recv(&mut self, mut data: Bytes) -> bool {
        assert!(!self.destroyed);

        while !data.is_empty() {
            if self.state == State::Payload {
                self.read_message(&mut data);
            } else {
                self.read_varint(&mut data);
            }
        }
        if self.state == State::Payload && self.length == 0 {
            self.read_message(&mut data);
        }

        !self.destroyed
    }

    fn read_message(&mut self, data: &mut Bytes) {
        let msg_missing = self.length.as_usize_checked().unwrap();
        if data.len() >= msg_missing {
            let msg_missing_bytes = data.split_to(msg_missing);
            if let Some(ref mut msg) = self.message {
                msg.put_slice(&msg_missing_bytes);
            } else {
                self.message = Some(msg_missing_bytes.into());
            }
            if !(self.next_state(data)) {
                data.clear()
            };
            return;
        }

        if self.message.is_none() {
            self.message = Some(BytesMut::with_capacity(msg_missing));
        }
        let msg = self.message.as_mut().unwrap();
        msg.put_slice(data);
        self.length -= data.len().as_u64_checked().unwrap();
        data.clear();
    }

    fn read_varint(&mut self, data: &mut Bytes) {
        while !data.is_empty() {
            self.varint += (data[0] & 127) as u64 * self.factor;
            self.consumed += 1;
            if data[0] < 128 {
                data.advance(1);
                if !(self.next_state(data)) {
                    data.clear();
                };
                return;
            }
            self.factor *= 128;
            data.advance(1);
        }
        if self.consumed >= 8 {
            self.destroy("Incoming varint is invalid".to_owned().into()); // 8 * 7bits is 56 ie max for js
        }
    }

    fn next_state(&mut self, data: &Bytes) -> bool {
        match self.state {
            State::Length => {
                self.state = State::Header;
                self.factor = 1;
                self.length = self.varint;
                self.varint = 0;
                self.consumed = 0;
                if self.length == 0 {
                    self.state = State::Length;
                }
                true
            }

            State::Header => {
                self.state = State::Payload;
                self.factor = 1;
                self.header = self.varint;
                self.length = self.length.checked_sub(self.consumed).unwrap();
                self.varint = 0;
                self.consumed = 0;
                if self.length.as_usize_checked().unwrap() > self.max_size {
                    self.destroy("Incoming message is larger than max size".to_owned().into());
                    return false;
                }

                let extra = data.len();
                let len = self.length.as_usize_checked().unwrap();
                if len > extra {
                    self.listener.on_missing(len - extra)
                }

                true
            }

            State::Payload => {
                self.state = State::Length;
                let msg = self.message.take().unwrap();
                self.on_message(
                    Channel(self.header >> 4),
                    Type(self.header & 0b1111),
                    msg.freeze(),
                );
                !self.destroyed
            }
        }
    }

    fn on_message(&mut self, channel: Channel, r#type: Type, message: Bytes) {
        self.listener.on_message(channel, r#type, message)
    }
}

#[derive(Eq, PartialEq)]
enum State {
    Length,
    Header,
    Payload,
}

pub trait Listener {
    fn on_message(&mut self, channel: Channel, r#type: Type, message: Bytes);
    fn on_missing(&mut self, len: usize);
}