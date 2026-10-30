#![cfg(all(feature = "futures-codec", feature = "derive"))]

use asynchronous_codec::{BytesMut, Decoder as _, Encoder as _, Framed, FramedRead, FramedWrite};
use futures::{SinkExt as _, StreamExt as _, executor::block_on, io::Cursor};
use thalassa::codec::TlsplCodec;
use thalassa::error::TlsplReadError;

#[derive(Debug, PartialEq, thalassa::TlsplAll)]
struct Frame {
    id: u16,
    payload: Vec<u8>,
    tags: Vec<u16>,
}

#[derive(Debug, PartialEq, thalassa::TlsplAll)]
#[tlspl(extensible)]
#[repr(u8)]
enum Ext {
    #[tlspl(discriminant = 1)]
    One(u16),
}

const MAX: usize = 1 << 20;

fn frames() -> Vec<Frame> {
    vec![
        Frame {
            id: 1,
            payload: vec![],
            tags: vec![],
        },
        Frame {
            id: 2,
            payload: vec![0xAB; 70_000],
            tags: vec![1, 2, 3],
        },
        Frame {
            id: 3,
            payload: b"tail".to_vec(),
            tags: vec![9],
        },
    ]
}

fn encode_all(items: &[Frame]) -> Vec<u8> {
    let mut codec = TlsplCodec::<&Frame>::new(MAX);
    let mut buf = BytesMut::new();
    for item in items {
        codec.encode(item, &mut buf).unwrap();
    }
    buf.to_vec()
}

/// Feeds `wire` in chunks of the given sizes (cycled), decoding after every chunk.
fn decode_chunked(wire: &[u8], sizes: impl Iterator<Item = usize>) -> Vec<Frame> {
    let mut codec = TlsplCodec::<Frame>::new(MAX);
    let mut buf = BytesMut::new();
    let mut out = vec![];
    let mut rest = wire;
    for size in sizes {
        if rest.is_empty() {
            break;
        }
        let (chunk, tail) = rest.split_at(size.min(rest.len()));
        rest = tail;
        buf.extend_from_slice(chunk);
        while let Some(frame) = codec.decode(&mut buf).unwrap() {
            out.push(frame);
        }
    }
    assert!(codec.decode_eof(&mut buf).unwrap().is_none());
    out
}

#[test]
fn one_byte_at_a_time() {
    let wire = encode_all(&frames());
    assert_eq!(decode_chunked(&wire, std::iter::repeat(1)), frames());
}

#[test]
fn random_splits_and_back_to_back() {
    let wire = encode_all(&frames());
    for seed in 1u64..50 {
        // xorshift: deterministic split sizes in 1..=4096
        let sizes = std::iter::successors(Some(seed), |s| {
            let mut x = *s;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            Some(x)
        })
        .map(|x| (x % 4096) as usize + 1);
        assert_eq!(decode_chunked(&wire, sizes), frames(), "seed {seed}");
    }
    // Whole stream in one buffer.
    assert_eq!(decode_chunked(&wire, std::iter::once(wire.len())), frames());
}

#[test]
fn truncated_stream_errors_once_at_eof() {
    let wire = encode_all(&frames());
    let mut codec = TlsplCodec::<Frame>::new(MAX);
    let mut buf = BytesMut::from(&wire[..wire.len() - 1]);
    assert_eq!(codec.decode(&mut buf).unwrap(), Some(frames().remove(0)));
    assert_eq!(codec.decode(&mut buf).unwrap(), Some(frames().remove(1)));
    assert_eq!(codec.decode(&mut buf).unwrap(), None);
    assert!(codec.decode_eof(&mut buf).unwrap_err().is_truncation());
    assert!(buf.is_empty());
    assert_eq!(codec.decode_eof(&mut buf).unwrap(), None);
}

#[test]
fn oversized_length_prefix_fails_before_buffering() {
    // id, then a 4-byte varint announcing a 0x0100_0000-byte payload: only the header is sent.
    let mut buf = BytesMut::from(&[0x00, 0x01, 0x81, 0x00, 0x00, 0x00][..]);
    let mut codec = TlsplCodec::<Frame>::new(MAX);
    assert!(matches!(
        codec.decode(&mut buf),
        Err(TlsplReadError::FrameTooLarge { max: MAX })
    ));
}

#[test]
fn complete_frame_over_the_limit_is_rejected() {
    let wire = encode_all(&frames());
    let mut codec = TlsplCodec::<Frame>::new(1024);
    let mut buf = BytesMut::from(&wire[..]);
    assert!(codec.decode(&mut buf).unwrap().is_some());
    assert!(matches!(
        codec.decode(&mut buf),
        Err(TlsplReadError::FrameTooLarge { max: 1024 })
    ));
}

#[test]
fn malformed_frame_is_terminal_and_poisons() {
    // `tags` declares 1 byte: its u16 element overruns it (ContentLengthMismatch).
    let mut wire = encode_all(&frames()[..1]);
    wire.extend_from_slice(&[0x00, 0x05, 0x00, 0x01, 0x07]); // id 5, empty payload, tags len 1
    wire.extend_from_slice(&encode_all(&frames()[2..]));
    let mut codec = TlsplCodec::<Frame>::new(MAX);
    let mut buf = BytesMut::from(&wire[..]);
    assert_eq!(codec.decode(&mut buf).unwrap(), Some(frames().remove(0)));
    let err = codec.decode(&mut buf).unwrap_err();
    assert!(!err.is_truncation(), "{err:?}");
    // Poisoned: later input is dropped, not parsed.
    buf.extend_from_slice(&encode_all(&frames()));
    assert_eq!(codec.decode(&mut buf).unwrap(), None);
    assert!(buf.is_empty());
}

#[test]
fn truncation_inside_complete_extensible_blob_is_terminal() {
    // Discriminant 1, then a complete 1-byte blob that is too short for the u16 inside.
    let mut buf = BytesMut::from(&[0x01, 0x01, 0xFF][..]);
    let mut codec = TlsplCodec::<Ext>::new(MAX);
    assert!(matches!(
        codec.decode(&mut buf),
        Err(TlsplReadError::NestedTruncation)
    ));

    // Same blob, well-formed, split across reads: still waits.
    let mut codec = TlsplCodec::<Ext>::new(MAX);
    let mut buf = BytesMut::from(&[0x01, 0x02, 0x12][..]);
    assert_eq!(codec.decode(&mut buf).unwrap(), None);
    buf.extend_from_slice(&[0x34]);
    assert_eq!(codec.decode(&mut buf).unwrap(), Some(Ext::One(0x1234)));
}

#[test]
fn zero_sized_item_is_rejected() {
    // `()` parses from zero bytes; without the guard this yields forever.
    let mut codec = TlsplCodec::<()>::new(MAX);
    assert!(matches!(
        codec.decode(&mut BytesMut::new()),
        Err(TlsplReadError::ZeroSizedFrame)
    ));

    // Same with data buffered.
    let mut codec = TlsplCodec::<()>::new(MAX);
    let mut buf = BytesMut::from(&[0xFF][..]);
    assert!(matches!(
        codec.decode(&mut buf),
        Err(TlsplReadError::ZeroSizedFrame)
    ));

    // Terminal, like any other codec error.
    assert_eq!(codec.decode(&mut buf).unwrap(), None);
    assert!(buf.is_empty());
}

/// Same wire format as [`Frame`], but borrowing: only encodable with a non-`'static` lifetime.
#[derive(thalassa::TlsplSize, thalassa::TlsplSerialize)]
struct BorrowedFrame<'a> {
    id: u16,
    payload: std::borrow::Cow<'a, [u8]>,
    tags: Vec<u16>,
}

#[test]
fn framed_round_trip() {
    block_on(async {
        let mut framed = Framed::new(Cursor::new(Vec::new()), TlsplCodec::<Frame>::new(MAX));
        for frame in frames() {
            framed.send(frame).await.unwrap();
        }
        let mut io = framed.into_inner();
        io.set_position(0);

        let decoded: Vec<Frame> = FramedRead::new(io, TlsplCodec::new(MAX))
            .map(Result::unwrap)
            .collect()
            .await;
        assert_eq!(decoded, frames());
    });
}

#[test]
fn encodes_borrowed_values_without_static_bound() {
    let owned = frames();
    let borrowed: Vec<BorrowedFrame<'_>> = owned
        .iter()
        .map(|f| BorrowedFrame {
            id: f.id,
            payload: (&f.payload[..]).into(),
            tags: f.tags.clone(),
        })
        .collect();
    block_on(async {
        let mut sink = FramedWrite::new(Vec::new(), TlsplCodec::<&BorrowedFrame<'_>>::new(MAX));
        for frame in &borrowed {
            sink.send(frame).await.unwrap();
        }
        assert_eq!(sink.into_inner(), encode_all(&owned));
    });
}

/// Writes four bytes, then fails.
struct FailsMidWrite;

impl thalassa::TlsplSize for FailsMidWrite {
    fn tlspl_serialized_len(&self) -> usize {
        4
    }
}

impl thalassa::TlsplSerialize for FailsMidWrite {
    fn tlspl_serialize_to<W: thalassa::io::Write>(
        &self,
        writer: &mut W,
    ) -> thalassa::error::TlsplWriteResult<usize> {
        writer.write_all(b"junk")?;
        Err(thalassa::error::TlsplWriteError::custom(
            "tests/codec",
            "boom",
        ))
    }
}

#[test]
fn encode_error_rolls_the_buffer_back() {
    let frames = frames();
    let first = encode_all(&frames[..1]);
    let mut buf = BytesMut::from(&first[..]);

    let mut failing = TlsplCodec::<FailsMidWrite>::new(MAX);
    assert!(failing.encode(FailsMidWrite, &mut buf).is_err());
    assert_eq!(&buf[..], &first[..], "partial frame left behind");
}
