use std::cell::RefCell;
use std::cmp::min;
use std::ops::Deref;
use std::rc::Rc;

use bytes::Bytes;
use integer_encoding::VarInt;
use simple_message_channels::{Channel, Listener, SimpleMessageChannels, Type};

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
    fn on_message(&mut self, channel: Channel, r#type: Type, message: Bytes) {}
    fn on_missing(&mut self, len: usize) {}
}

#[test]
fn basic() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut a = SimpleMessageChannels::new(None, CollectingListener(events.clone()));
    let bytes = a.send(Channel(0), Type(1), &Bytes::from("hi"));
    a.recv(bytes);

    drop(a);
    assert_eq!(
        Rc::try_unwrap(events).unwrap().into_inner(),
        vec![Event::Message(Channel(0), Type(1), Bytes::from("hi"))]
    );
}

#[test]
fn basic_chunked() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut a = SimpleMessageChannels::new(None, CollectingListener(events.clone()));

    let payload = a.send(Channel(0), Type(1), &Bytes::from("hi"));

    for i in 0..payload.len() {
        a.recv(payload.slice(i, i + 1));
    }

    drop(a);
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
    let mut a = SimpleMessageChannels::new(None, CollectingListener(events.clone()));

    let payload = a.send(Channel(0), Type(1), &Bytes::from("hi"));

    for i in 0..payload.len() {
        a.recv(payload.slice(i, i + 1));
    }

    let payload2 = a.send(Channel(42), Type(3), &Bytes::from("hey"));

    for i in 0..payload2.len() {
        a.recv(payload2.slice(i, i + 1));
    }

    drop(a);
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
    let mut a = SimpleMessageChannels::new(None, CollectingListener(events.clone()));

    const LEN1: usize = 100_000;
    const LEN2: usize = 200_000;
    const STEP: usize = 500;

    let payload = a.send(Channel(0), Type(1), &Bytes::from([22; LEN1].as_ref()));

    for i in (0..payload.len()).step_by(STEP) {
        a.recv(payload.slice(i, min(i + STEP, payload.len())));
    }

    let payload2 = a.send(Channel(42), Type(3), &Bytes::from([33; LEN2].as_ref()));

    for i in (0..payload2.len()).step_by(STEP) {
        a.recv(payload2.slice(i, min(i + STEP, payload2.len())));
    }

    drop(a);
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
    let mut a = SimpleMessageChannels::new(None, CollectingListener(events.clone()));

    let bytes = a.send(Channel(0), Type(0), &Bytes::new());
    a.recv(bytes);

    drop(a);
    assert_eq!(
        Rc::try_unwrap(events).unwrap().into_inner(),
        vec![Event::Message(Channel(0), Type(0), Bytes::new())]
    );
}

#[test]
fn chunk_message_is_correct() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut a = SimpleMessageChannels::new(None, CollectingListener(events.clone()));
    let mut b = SimpleMessageChannels::new(None, NopListener);

    let payload = b.send(Channel(0), Type(1), &Bytes::from("aaaaaaaaaa"));

    a.recv(payload.slice(0, 4));
    a.recv(payload.slice(4, 12));

    drop(a);
    assert_eq!(
        Rc::try_unwrap(events).unwrap().into_inner(),
        vec![
            Event::Missing(8),
            Event::Message(Channel(0), Type(1), Bytes::from("aaaaaaaaaa"))
        ]
    );
}