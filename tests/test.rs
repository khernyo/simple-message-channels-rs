use std::cmp::min;

use bytes::Bytes;
use integer_encoding::VarInt;
use simple_message_channels::{Channel, Decoder, Encoder, Event, Message, MessageType};

#[test]
fn basic() {
    let mut decoder = Decoder::new(None);
    let mut encoder = Encoder::new(None);
    let bytes = encoder
        .send(Channel(0), MessageType(1), &Bytes::from("hi"))
        .unwrap();

    let events: Vec<Event> = decoder.messages(bytes).collect();

    assert_eq!(
        events,
        vec![Event::Message(Message {
            channel: Channel(0),
            message_type: MessageType(1),
            data: Bytes::from("hi"),
        })]
    );
}

#[test]
fn basic_chunked() {
    let mut decoder = Decoder::new(None);
    let mut encoder = Encoder::new(None);

    let payload = encoder
        .send(Channel(0), MessageType(1), &Bytes::from("hi"))
        .unwrap();

    let mut events: Vec<Event> = vec![];
    for i in 0..payload.len() {
        events.extend(decoder.messages(payload.slice(i, i + 1)));
    }

    assert_eq!(
        events,
        vec![
            Event::Missing(2),
            Event::Message(Message {
                channel: Channel(0),
                message_type: MessageType(1),
                data: Bytes::from("hi"),
            }),
        ]
    );
}

#[test]
fn two_messages_chunked() {
    let mut decoder = Decoder::new(None);
    let mut encoder = Encoder::new(None);

    let payload = encoder
        .send(Channel(0), MessageType(1), &Bytes::from("hi"))
        .unwrap();

    let mut events: Vec<Event> = vec![];
    for i in 0..payload.len() {
        events.extend(decoder.messages(payload.slice(i, i + 1)));
    }

    let payload2 = encoder
        .send(Channel(42), MessageType(3), &Bytes::from("hey"))
        .unwrap();

    for i in 0..payload2.len() {
        events.extend(decoder.messages(payload2.slice(i, i + 1)));
    }

    assert_eq!(
        events,
        vec![
            Event::Missing(2),
            Event::Message(Message {
                channel: Channel(0),
                message_type: MessageType(1),
                data: Bytes::from("hi"),
            }),
            Event::Missing(3),
            Event::Message(Message {
                channel: Channel(42),
                message_type: MessageType(3),
                data: Bytes::from("hey"),
            }),
        ]
    );
}

#[test]
fn two_big_messages_chunked() {
    let mut decoder = Decoder::new(None);
    let mut encoder = Encoder::new(None);

    const LEN1: usize = 100_000;
    const LEN2: usize = 200_000;
    const STEP: usize = 500;

    let payload = encoder
        .send(
            Channel(0),
            MessageType(1),
            &Bytes::from([22; LEN1].as_ref()),
        )
        .unwrap();

    let mut events: Vec<Event> = vec![];
    for i in (0..payload.len()).step_by(STEP) {
        events.extend(decoder.messages(payload.slice(i, min(i + STEP, payload.len()))));
    }

    let payload2 = encoder
        .send(
            Channel(42),
            MessageType(3),
            &Bytes::from([33; LEN2].as_ref()),
        )
        .unwrap();

    for i in (0..payload2.len()).step_by(STEP) {
        events.extend(decoder.messages(payload2.slice(i, min(i + STEP, payload2.len()))));
    }

    assert_eq!(
        events,
        vec![
            Event::Missing(LEN1 - STEP + encoding_length(Channel(0), MessageType(1), LEN1)),
            Event::Message(Message {
                channel: Channel(0),
                message_type: MessageType(1),
                data: Bytes::from([22; LEN1].as_ref()),
            }),
            Event::Missing(LEN2 - STEP + encoding_length(Channel(42), MessageType(3), LEN2)),
            Event::Message(Message {
                channel: Channel(42),
                message_type: MessageType(3),
                data: Bytes::from([33; LEN2].as_ref()),
            }),
        ]
    );
}

#[test]
fn two_big_messages_in_three_chunkes() {
    let mut decoder = Decoder::new(None);
    let mut encoder = Encoder::new(None);

    const CHUNKS: usize = 3;
    const LEN1: usize = 100_000;
    const LEN2: usize = 150_000;
    const STEP: usize = 90_000;

    let mut payload = encoder
        .send(
            Channel(0),
            MessageType(1),
            &Bytes::from([22; LEN1].as_ref()),
        )
        .unwrap();
    payload.extend_from_slice(
        &encoder
            .send(
                Channel(42),
                MessageType(3),
                &Bytes::from([33; LEN2].as_ref()),
            )
            .unwrap(),
    );

    let mut chunks = Vec::<Bytes>::new();
    for i in (0..payload.len()).step_by(STEP) {
        chunks.push(payload.slice(i, min(i + STEP, payload.len())));
    }
    assert_eq!(chunks.len(), CHUNKS);

    let mut events: Vec<Vec<Event>> = vec![];
    for chunk in chunks {
        events.push(decoder.messages(chunk).collect());
    }

    let len1 = LEN1 + encoding_length(Channel(0), MessageType(1), LEN1);
    let len2 = LEN2 + encoding_length(Channel(42), MessageType(3), LEN2);
    assert_eq!(
        events,
        vec![
            vec![Event::Missing(len1 - STEP)],
            vec![
                Event::Message(Message {
                    channel: Channel(0),
                    message_type: MessageType(1),
                    data: Bytes::from([22; LEN1].as_ref()),
                }),
                Event::Missing((len1 + len2) - 2 * STEP),
            ],
            vec![Event::Message(Message {
                channel: Channel(42),
                message_type: MessageType(3),
                data: Bytes::from([33; LEN2].as_ref()),
            })]
        ]
    );
}

fn encoding_length(channel: Channel, message_type: MessageType, data_len: usize) -> usize {
    VarInt::required_space(data_len) + VarInt::required_space(channel.0 << 4 | message_type.0)
}

#[test]
fn empty_message() {
    let mut decoder = Decoder::new(None);
    let mut encoder = Encoder::new(None);

    let bytes = encoder
        .send(Channel(0), MessageType(0), &Bytes::new())
        .unwrap();
    let events: Vec<Event> = decoder.messages(bytes).collect();

    assert_eq!(
        events,
        vec![Event::Message(Message {
            channel: Channel(0),
            message_type: MessageType(0),
            data: Bytes::new(),
        })]
    );
}

#[test]
fn chunk_message_is_correct() {
    let mut decoder = Decoder::new(None);
    let mut encoder = Encoder::new(None);

    let payload = encoder
        .send(Channel(0), MessageType(1), &Bytes::from("aaaaaaaaaa"))
        .unwrap();

    let mut events: Vec<Event> = decoder.messages(payload.slice(0, 4)).collect();
    events.extend(decoder.messages(payload.slice(4, 12)));

    assert_eq!(
        events,
        vec![
            Event::Missing(8),
            Event::Message(Message {
                channel: Channel(0),
                message_type: MessageType(1),
                data: Bytes::from("aaaaaaaaaa"),
            }),
        ]
    );
}

#[test]
fn large_message() {
    let mut decoder = Decoder::new(2);
    let mut encoder = Encoder::new(2);

    let mut events: Vec<Event> = vec![];
    let bytes = encoder
        .send(Channel(0), MessageType(1), &Bytes::from("hi"))
        .unwrap();
    events.extend(decoder.messages(bytes));

    let bytes = encoder
        .send(Channel(0), MessageType(1), &Bytes::from("12"))
        .unwrap();
    events.extend(decoder.messages(bytes));

    assert!(encoder
        .send(Channel(1), MessageType(2), &Bytes::from("foo"))
        .is_err());

    assert_eq!(
        events,
        vec![
            Event::Message(Message {
                channel: Channel(0),
                message_type: MessageType(1),
                data: Bytes::from("hi"),
            }),
            Event::Message(Message {
                channel: Channel(0),
                message_type: MessageType(1),
                data: Bytes::from("12"),
            }),
        ]
    );
}
