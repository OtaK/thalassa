#![allow(dead_code)]
//! A `#[tlspl(select = ...)]` source that is *wider* than the target enum's `#[repr]` must not
//! compile: the discriminant is narrowed into that repr, so a wider source would be truncated
//! silently. The check runs after monomorphization, in `assert_select_discriminant_fits`.
use thalassa::TlsplDeserialize;

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u8)]
enum Narrow {
    N1 = 0x01,
}

#[derive(Debug, thalassa::TlsplAll)]
#[tlspl(untagged)]
#[repr(u8)]
enum Content {
    #[tlspl(discriminant = "Narrow::N1")]
    A { t: bool },
}

#[derive(Debug, Clone, Copy, thalassa::TlsplAll)]
#[repr(u16)]
enum WideDiscr {
    W1 = 0x0301,
}

#[derive(Debug, thalassa::TlsplAll)]
struct Message {
    d: WideDiscr,
    #[tlspl(select = self.d)]
    c: Content,
}

fn main() {
    let data = &[0x03u8, 0x01, 0x01];
    let mut reader = thalassa::io::DepthAwareReader::from_reader(&data[..]);
    let mut guard = reader.enter().unwrap();
    let _ = Message::tlspl_deserialize_from_complete(&mut guard);
}
