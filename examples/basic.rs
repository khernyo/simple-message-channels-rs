use bytes::Bytes;
use simple_message_channels::{Channel, Decoder, Encoder, MessageType};

fn main() {
    let mut decoder = Decoder::new(None);
    let mut encoder = Encoder::new(None);

    let mut bytes = encoder
        .send(Channel(0), MessageType(1), &Bytes::from(b"a".as_ref()))
        .unwrap();
    bytes.extend_from_slice(
        &encoder
            .send(Channel(0), MessageType(1), &Bytes::from(b"b".as_ref()))
            .unwrap(),
    );
    bytes.extend_from_slice(
        &encoder
            .send(Channel(0), MessageType(1), &Bytes::from(b"c".as_ref()))
            .unwrap(),
    );

    for msg in decoder.messages(bytes) {
        println!("{:?}", msg);
    }
}
