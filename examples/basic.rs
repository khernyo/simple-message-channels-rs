use bytes::Bytes;
use simple_message_channels::{Channel, Listener, SimpleMessageChannels, Type};

fn main() {
    let mut a = SimpleMessageChannels::new(None, PrintingListener("a"));
    let mut b = SimpleMessageChannels::new(None, PrintingListener("b"));

    let mut bytes = b.send(Channel(0), Type(1), &Bytes::from(b"a".as_ref()));
    bytes.extend_from_slice(&b.send(Channel(0), Type(1), &Bytes::from(b"b".as_ref())));
    bytes.extend_from_slice(&b.send(Channel(0), Type(1), &Bytes::from(b"c".as_ref())));

    let x = a.recv(bytes);
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
