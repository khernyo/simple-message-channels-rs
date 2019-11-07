use bytes::Bytes;
use simple_message_channels::{Channel, Decoder, Encoder, Listener, Type};

fn main() {
    let mut decoder = Decoder::new(None, PrintingListener("a"));
    let mut encoder = Encoder::new(None);

    let mut bytes = encoder
        .send(Channel(0), Type(1), &Bytes::from(b"a".as_ref()))
        .unwrap();
    bytes.extend_from_slice(
        &encoder
            .send(Channel(0), Type(1), &Bytes::from(b"b".as_ref()))
            .unwrap(),
    );
    bytes.extend_from_slice(
        &encoder
            .send(Channel(0), Type(1), &Bytes::from(b"c".as_ref()))
            .unwrap(),
    );

    let x = decoder.recv(bytes);
    println!("{:?}", x);
}

struct PrintingListener(&'static str);

impl Listener for PrintingListener {
    fn on_message(&mut self, channel: Channel, r#type: Type, message: Bytes) {
        println!(
            "{:?} onmessage: channel:{:?} type:{:?} message:{:?}",
            self.0, channel, r#type, message
        );
    }

    fn on_missing(&mut self, count: usize) {
        println!("{:?} onmissing: count:{}", self.0, count);
    }
}
