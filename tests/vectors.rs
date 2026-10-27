//! Wire vectors for the variable-length prefix.
//!
//! [`CANONICAL`] is the `deserialization.json` table from
//! <https://github.com/mlswg/mls-implementations/tree/main/test-vectors> (the RFC 9420 / RFC 9000
//! varint), verbatim. [`NON_CANONICAL`] spells some of the same values in a longer form: they are
//! rejected so that a vector has exactly one encoding.
use thalassa::error::TlsplReadError;
use thalassa::io::DepthAwareReader;
use thalassa::types::ContentLengthLength;

/// `(prefix, value)` pairs.
const CANONICAL: &[(&[u8], usize)] = &[
    (&[0x00], 0),
    (&[0x0d], 13),
    (&[0x36], 54),
    (&[0x3f], 63),
    (&[0x40, 0x40], 64),
    (&[0x40, 0xff], 255),
    (&[0x41, 0x85], 389),
    (&[0x4a, 0xaa], 2730),
    (&[0x4f, 0xff], 4095),
    (&[0x7f, 0xff], 16383),
    (&[0x80, 0x00, 0x40, 0x00], 16384),
    (&[0x80, 0x00, 0xbe, 0xef], 48879),
    (&[0x80, 0x00, 0xde, 0xad], 57005),
    (&[0xbf, 0xff, 0xff, 0xff], 1_073_741_823),
];

/// `(prefix, value)`: a form longer than the value needs.
const NON_CANONICAL: &[(&[u8], usize)] = &[
    (&[0x40, 0x00], 0),
    (&[0x40, 0x3f], 63),
    (&[0x80, 0x00, 0x00, 0x40], 64),
    (&[0x80, 0x00, 0x3f, 0xff], 16383),
    (&[0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], 0),
    (&[0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01], 1),
];

fn read(prefix: &[u8]) -> Result<usize, TlsplReadError> {
    let mut reader = DepthAwareReader::from_reader(prefix);
    let mut guard = reader.enter().unwrap();
    ContentLengthLength::read_content_len(&mut guard)
}

fn write(value: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let written = ContentLengthLength::from_content_len(value)
        .write_content_len(value, &mut out)
        .unwrap();
    assert_eq!(written, out.len(), "prefix length reported for {value}");
    out
}

#[test]
fn length_prefix_vectors_round_trip() {
    for &(prefix, value) in CANONICAL {
        assert_eq!(read(prefix).unwrap(), value, "decoding {prefix:02x?}");
        assert_eq!(write(value), prefix, "encoding {value}");
    }
}

#[test]
fn non_canonical_length_prefixes_are_rejected() {
    for &(prefix, value) in NON_CANONICAL {
        let got = read(prefix);
        assert!(
            matches!(got, Err(TlsplReadError::NonCanonicalLength { value: got }) if got == value),
            "{prefix:02x?} should be rejected as non-canonical, got {got:?}"
        );
        assert_ne!(
            write(value),
            prefix,
            "the encoder emits {value} canonically"
        );
    }
}

#[test]
fn length_prefix_past_the_30_bit_cap_errors_as_an_overflow() {
    // 2^30 is canonically an 8-byte prefix, past the `MAX_LEN` this codec admits.
    let prefix = [0xc0, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00];
    assert!(matches!(read(&prefix), Err(TlsplReadError::LengthOverflow)));
}
