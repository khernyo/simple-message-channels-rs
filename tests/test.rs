use std::cell::RefCell;
use std::cmp::min;
use std::rc::Rc;

use bytes::Bytes;
use integer_encoding::VarInt;
use simple_message_channels::{Channel, Decoder, Encoder, Listener, Type};

#[derive(Debug, Eq, PartialEq)]
enum Event {
    Message(Channel, Type, Bytes),
    Missing(usize),
}

#[derive(Debug, Eq, PartialEq)]
struct CollectingListener(Rc<RefCell<Vec<Event>>>);

impl Listener for CollectingListener {
    fn on_message(&mut self, channel: Channel, r#type: Type, message: Bytes) {
        self.0
            .borrow_mut()
            .push(Event::Message(channel, r#type, message));
    }

    fn on_missing(&mut self, len: usize) {
        self.0.borrow_mut().push(Event::Missing(len));
    }
}

struct NopListener;
impl Listener for NopListener {
    fn on_message(&mut self, _: Channel, _: Type, _: Bytes) {}
    fn on_missing(&mut self, _len: usize) {}
}

#[test]
fn basic() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut decoder = Decoder::new(None, CollectingListener(events.clone()));
    let mut encoder = Encoder::new(None);
    let bytes = encoder
        .send(Channel(0), Type(1), &Bytes::from("hi"))
        .unwrap();
    decoder.recv(bytes);

    drop(decoder);
    assert_eq!(
        Rc::try_unwrap(events).unwrap().into_inner(),
        vec![Event::Message(Channel(0), Type(1), Bytes::from("hi"))]
    );
}

#[test]
fn basic_chunked() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut decoder = Decoder::new(None, CollectingListener(events.clone()));
    let mut encoder = Encoder::new(None);

    let payload = encoder
        .send(Channel(0), Type(1), &Bytes::from("hi"))
        .unwrap();

    for i in 0..payload.len() {
        decoder.recv(payload.slice(i, i + 1));
    }

    drop(decoder);
    assert_eq!(
        Rc::try_unwrap(events).unwrap().into_inner(),
        vec![
            Event::Missing(2),
            Event::Message(Channel(0), Type(1), Bytes::from("hi"))
        ]
    );
}

#[test]
fn two_messages_chunked() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut decoder = Decoder::new(None, CollectingListener(events.clone()));
    let mut encoder = Encoder::new(None);

    let payload = encoder
        .send(Channel(0), Type(1), &Bytes::from("hi"))
        .unwrap();

    for i in 0..payload.len() {
        decoder.recv(payload.slice(i, i + 1));
    }

    let payload2 = encoder
        .send(Channel(42), Type(3), &Bytes::from("hey"))
        .unwrap();

    for i in 0..payload2.len() {
        decoder.recv(payload2.slice(i, i + 1));
    }

    drop(decoder);
    assert_eq!(
        Rc::try_unwrap(events).unwrap().into_inner(),
        vec![
            Event::Missing(2),
            Event::Message(Channel(0), Type(1), Bytes::from("hi")),
            Event::Missing(3),
            Event::Message(Channel(42), Type(3), Bytes::from("hey")),
        ]
    );
}

#[test]
fn two_big_messages_chunked() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut decoder = Decoder::new(None, CollectingListener(events.clone()));
    let mut encoder = Encoder::new(None);

    const LEN1: usize = 100_000;
    const LEN2: usize = 200_000;
    const STEP: usize = 500;

    let payload = encoder
        .send(Channel(0), Type(1), &Bytes::from([22; LEN1].as_ref()))
        .unwrap();

    for i in (0..payload.len()).step_by(STEP) {
        decoder.recv(payload.slice(i, min(i + STEP, payload.len())));
    }

    let payload2 = encoder
        .send(Channel(42), Type(3), &Bytes::from([33; LEN2].as_ref()))
        .unwrap();

    for i in (0..payload2.len()).step_by(STEP) {
        decoder.recv(payload2.slice(i, min(i + STEP, payload2.len())));
    }

    drop(decoder);
    assert_eq!(
        Rc::try_unwrap(events).unwrap().into_inner(),
        vec![
            Event::Missing(LEN1 - STEP + encoding_length(Channel(0), Type(1), LEN1)),
            Event::Message(Channel(0), Type(1), Bytes::from([22; LEN1].as_ref())),
            Event::Missing(LEN2 - STEP + encoding_length(Channel(42), Type(3), LEN2)),
            Event::Message(Channel(42), Type(3), Bytes::from([33; LEN2].as_ref())),
        ]
    );
}

fn encoding_length(channel: Channel, r#type: Type, data_len: usize) -> usize {
    VarInt::required_space(data_len) + VarInt::required_space(channel.0 << 4 | r#type.0)
}

#[test]
fn empty_message() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut decoder = Decoder::new(None, CollectingListener(events.clone()));
    let mut encoder = Encoder::new(None);

    let bytes = encoder.send(Channel(0), Type(0), &Bytes::new()).unwrap();
    decoder.recv(bytes);

    drop(decoder);
    assert_eq!(
        Rc::try_unwrap(events).unwrap().into_inner(),
        vec![Event::Message(Channel(0), Type(0), Bytes::new())]
    );
}

#[test]
fn chunk_message_is_correct() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut decoder = Decoder::new(None, CollectingListener(events.clone()));
    let mut encoder = Encoder::new(None);

    let payload = encoder
        .send(Channel(0), Type(1), &Bytes::from("aaaaaaaaaa"))
        .unwrap();

    decoder.recv(payload.slice(0, 4));
    decoder.recv(payload.slice(4, 12));

    drop(decoder);
    assert_eq!(
        Rc::try_unwrap(events).unwrap().into_inner(),
        vec![
            Event::Missing(8),
            Event::Message(Channel(0), Type(1), Bytes::from("aaaaaaaaaa"))
        ]
    );
}

#[test]
fn large_message() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut decoder = Decoder::new(2, CollectingListener(events.clone()));
    let mut encoder = Encoder::new(2);
    let bytes = encoder
        .send(Channel(0), Type(1), &Bytes::from("hi"))
        .unwrap();
    decoder.recv(bytes);
    let bytes = encoder
        .send(Channel(0), Type(1), &Bytes::from("12"))
        .unwrap();
    decoder.recv(bytes);
    assert!(encoder
        .send(Channel(1), Type(2), &Bytes::from("foo"))
        .is_err());

    drop(decoder);
    assert_eq!(
        Rc::try_unwrap(events).unwrap().into_inner(),
        vec![
            Event::Message(Channel(0), Type(1), Bytes::from("hi")),
            Event::Message(Channel(0), Type(1), Bytes::from("12"))
        ]
    );
}
