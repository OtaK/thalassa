#![no_main]

use asynchronous_codec::{BytesMut, Decoder as _};
use libfuzzer_sys::fuzz_target;
use thalassa::codec::TlsplCodec;
use thalassa::{TlsplDeserialize, TlsplSerialize};

type Item = Vec<Vec<u8>>;

// Splitting a valid encoding anywhere must decode to the same values as the one-shot slice parse.
fuzz_target!(|input: (Vec<Item>, Vec<u16>)| {
    let (items, splits) = input;
    let mut wire = vec![];
    for item in &items {
        let Ok(bytes) = item.tlspl_serialize() else {
            return;
        };
        wire.extend_from_slice(&bytes);
    }

    let mut expected = vec![];
    let mut slice = wire.as_slice();
    while !slice.is_empty() {
        let mut reader = thalassa::io::DepthAwareReader::from_reader(&mut slice);
        let mut guard = reader.enter().expect("enter");
        expected.push(Item::tlspl_deserialize_from(&mut guard).expect("valid encoding"));
    }
    assert_eq!(expected, items);

    let mut codec = TlsplCodec::<Item>::new(usize::MAX);
    let mut buf = BytesMut::new();
    let mut decoded = vec![];
    let mut rest = wire.as_slice();
    let mut sizes = splits.iter().map(|&s| s as usize + 1).chain(std::iter::repeat(usize::MAX));
    while !rest.is_empty() {
        let (chunk, tail) = rest.split_at(sizes.next().unwrap().min(rest.len()));
        rest = tail;
        buf.extend_from_slice(chunk);
        while let Some(v) = codec.decode(&mut buf).expect("valid encoding") {
            decoded.push(v);
        }
    }
    assert!(codec.decode_eof(&mut buf).expect("complete stream").is_none());
    assert_eq!(decoded, expected);
});
