use std::io::Write;

use bytes::{BufMut, Bytes, BytesMut};
use checked_int_cast::CheckedIntCast;
use integer_encoding::{VarInt, VarIntWriter};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Channel(pub u64);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Type(pub u64);

type UnknownNumber = i64; // TODO

pub struct SimpleMessageChannels<L> {
    listener: L,

    destroyed: bool,
    error: Option<String>,

    _message: Option<BytesMut>,
    _ptr: UnknownNumber,
    _varint: u64,
    _factor: u64,
    _length: u64,
    _header: u64,
    _state: UnknownNumber,
    _consumed: u64,
    _max_size: usize,
}

impl<L: Listener> SimpleMessageChannels<L> {
    pub fn new<MS>(max_size: MS, listener: L) -> SimpleMessageChannels<L>
    where
        MS: Into<Option<usize>>,
    {
        let max_size = max_size.into().unwrap_or(8 * 1024 * 1024);

        SimpleMessageChannels {
            listener,

            destroyed: false,
            error: None,

            _message: None,
            _ptr: 0,
            _varint: 0,
            _factor: 1,
            _length: 0,
            _header: 0,
            _state: 0,
            _consumed: 0,
            _max_size: max_size,
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
            if self._state == 2 {
                self._read_message(&mut data);
            } else {
                self._read_varint(&mut data);
            }
        }
        if self._state == 2 && self._length == 0 {
            self._read_message(&mut data);
        }

        !self.destroyed
    }

    fn _read_message(&mut self, data: &mut Bytes) {
        let msg_missing = self._length.as_usize_checked().unwrap();
        if data.len() >= msg_missing {
            let msg_missing_bytes = data.split_to(msg_missing);
            if let Some(ref mut msg) = self._message {
                msg.put_slice(&msg_missing_bytes);
            } else {
                self._message = Some(msg_missing_bytes.into());
            }
            if !(self._next_state(data)) {
                data.clear()
            };
            return;
        }

        if self._message.is_none() {
            self._message = Some(BytesMut::with_capacity(msg_missing));
        }
        let msg = self._message.as_mut().unwrap();
        msg.put_slice(data);
        self._length -= data.len().as_u64_checked().unwrap();
        data.clear();
    }

    fn _read_varint(&mut self, data: &mut Bytes) {
        while !data.is_empty() {
            self._varint += (data[0] & 127) as u64 * self._factor;
            self._consumed += 1;
            if data[0] < 128 {
                data.advance(1);
                if !(self._next_state(data)) {
                    data.clear();
                };
                return;
            }
            self._factor *= 128;
            data.advance(1);
        }
        if self._consumed >= 8 {
            self.destroy("Incoming varint is invalid".to_owned().into()); // 8 * 7bits is 56 ie max for js
        }
    }

    fn _next_state(&mut self, data: &Bytes) -> bool {
        match self._state {
            0 => {
                self._state = 1;
                self._factor = 1;
                self._length = self._varint;
                self._varint = 0;
                self._consumed = 0;
                if self._length == 0 {
                    self._state = 0;
                }
                return true;
            }

            1 => {
                self._state = 2;
                self._factor = 1;
                self._header = self._varint;
                self._length = self._length.checked_sub(self._consumed).unwrap();
                self._varint = 0;
                self._consumed = 0;
                if self._length.as_usize_checked().unwrap() > self._max_size {
                    self.destroy("Incoming message is larger than max size".to_owned().into());
                    return false;
                }

                let extra = data.len();
                let len = self._length.as_usize_checked().unwrap();
                if len > extra {
                    self.listener.on_missing(len - extra)
                }

                return true;
            }

            2 => {
                self._state = 0;
                let msg = self._message.take().unwrap();
                self._onmessage(
                    Channel(self._header >> 4),
                    Type(self._header & 0b1111),
                    msg.freeze(),
                );
                return !self.destroyed;
            }

            _ => {
                return false;
            }
        }
    }

    fn _onmessage(&mut self, channel: Channel, r#type: Type, message: Bytes) {
        return self.listener.on_message(channel, r#type, message);
    }

    pub fn send(&mut self, channel: Channel, r#type: Type, data: &Bytes) -> Bytes {
        assert!(!self.destroyed);
        let header = channel.0 << 4 | r#type.0;
        let length = data.len() + VarInt::required_space(header);
        let payload = BytesMut::with_capacity(VarInt::required_space(length) + length);

        let mut writer = payload.writer();
        writer.write_varint(length).unwrap();
        writer.write_varint(header).unwrap();
        writer.write_all(data).unwrap();

        return writer.into_inner().freeze();
    }
}

pub trait Listener {
    fn on_message(&mut self, channel: Channel, r#type: Type, message: Bytes);
    fn on_missing(&mut self, len: usize);
}
