#![allow(dead_code)]

//! The spellings of `discriminant`: literals, unquoted expressions (emitted into a `#[repr]`-typed
//! const, so the compiler checks them) and quoted paths into another enum (read out of that enum's
//! representation). `main()` pins the value each spelling produces on the wire, in both directions.

use thalassa::io::DepthAwareReader;
use thalassa::{TlsplDeserialize, TlsplSerialize};

const CASE_A: u8 = 0x12;

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u8)]
enum ValueForms {
    #[tlspl(discriminant = CASE_A)]
    Variant1 {
        thing: bool,
    },
    #[tlspl(discriminant = 1 + 1)]
    Variant2,
    // implicit: follows the expression above (`Variant2 + 1`)
    Variant3,
    #[tlspl(discriminant = u8::MAX)]
    Variant4,
}

#[repr(u16)]
enum Source {
    S1 = 0x34,
    S2 = 0x35,
}

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u16)]
enum PathForms {
    #[tlspl(discriminant = "Source::S1")]
    Variant1 { thing: bool },
    #[tlspl(discriminant = "Source::S2")]
    Variant2,
}

/// A *native* discriminant written as an expression — no `#[tlspl]` attribute anywhere, so the
/// derive has to take the value from the enum body. `B` must encode as 5, rustc's numbering, not as
/// 1, which is where the derive would have landed had it dropped the expression and counted the
/// variant as implicit.
const NATIVE_FIVE: u8 = 5;

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u8)]
enum NativeForms {
    A,
    B = NATIVE_FIVE,
    // implicit: follows the expression above (`B + 1`)
    C,
}

fn bytes<T: TlsplSerialize>(value: &T) -> Vec<u8> {
    let bytes = value.tlspl_serialize().unwrap();
    assert_eq!(
        bytes.len(),
        value.tlspl_serialized_len(),
        "`tlspl_serialized_len` disagreed with `tlspl_serialize`"
    );
    bytes
}

fn decode<T: TlsplDeserialize<'static> + 'static>(bytes: &'static [u8]) -> T {
    let mut reader = DepthAwareReader::from_reader(bytes);
    let mut guard = reader.enter().unwrap();
    T::tlspl_deserialize_from(&mut guard).unwrap()
}

fn main() {
    // `ValueForms`: attribute expressions, plus an implicit variant that follows one.
    assert_eq!(bytes(&ValueForms::Variant1 { thing: true }), [CASE_A, 0x01]);
    assert_eq!(bytes(&ValueForms::Variant2), [1 + 1]);
    assert_eq!(bytes(&ValueForms::Variant3), [1 + 1 + 1]);
    assert_eq!(bytes(&ValueForms::Variant4), [u8::MAX]);

    // `PathForms`: the quoted path is read out of `Source`'s representation — `repr(u16)`,
    // big-endian, so the discriminant is two bytes.
    assert_eq!(
        bytes(&PathForms::Variant1 { thing: true }),
        [0x00, 0x34, 0x01]
    );
    assert_eq!(bytes(&PathForms::Variant2), [0x00, 0x35]);

    // `NativeForms`: the native expression, and the implicit variant after it.
    assert_eq!(bytes(&NativeForms::A), [0]);
    assert_eq!(bytes(&NativeForms::B), [NATIVE_FIVE]);
    assert_eq!(bytes(&NativeForms::C), [NATIVE_FIVE + 1]);

    // The read path agrees with the write path.
    assert!(matches!(
        decode::<ValueForms>(&[CASE_A, 0x01]),
        ValueForms::Variant1 { thing: true }
    ));
    assert!(matches!(decode::<ValueForms>(&[3]), ValueForms::Variant3));
    assert!(matches!(
        decode::<ValueForms>(&[u8::MAX]),
        ValueForms::Variant4
    ));
    assert!(matches!(
        decode::<PathForms>(&[0x00, 0x35]),
        PathForms::Variant2
    ));
    assert!(matches!(
        decode::<NativeForms>(&[NATIVE_FIVE]),
        NativeForms::B
    ));
    assert!(matches!(
        decode::<NativeForms>(&[NATIVE_FIVE + 1]),
        NativeForms::C
    ));
}
