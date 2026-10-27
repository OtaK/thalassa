use thalassa::{TlsplDeserialize, TlsplSerialize};

use crate::common::btd;

mod common;

#[derive(Debug, Clone, Copy, PartialEq, thalassa::TlsplAll)]
#[repr(u8)]
enum Discr {
    D1 = 0x01,
    D2 = 0x02,
}

#[derive(Debug, PartialEq, thalassa::TlsplAll)]
#[tlspl(untagged)]
#[repr(u8)]
enum Content {
    #[tlspl(discriminant = "Discr::D1")]
    A { t: bool },
    #[tlspl(discriminant = "Discr::D2")]
    B { n: u16 },
}

#[derive(Debug, PartialEq, thalassa::TlsplAll)]
struct Named {
    d: Discr,
    #[tlspl(select = self.d)]
    c: Content,
}

#[derive(Debug, PartialEq, thalassa::TlsplAll)]
#[repr(u8)]
enum Tuple {
    V(Discr, #[tlspl(select = self.0)] Content),
}

fn ser<T: TlsplSerialize>(value: &T) -> Vec<u8> {
    value.tlspl_serialize().expect("serialize")
}

fn de<'a, T: TlsplDeserialize<'a> + 'a>(bytes: &'a [u8]) -> T {
    T::tlspl_deserialize_from_complete(&mut btd(bytes).enter().unwrap()).expect("deserialize")
}

#[test]
fn named_member_carries_the_discriminant_once() {
    let a = Named {
        d: Discr::D1,
        c: Content::A { t: true },
    };
    assert_eq!(ser(&a), vec![0x01, 0x01]);
    assert_eq!(de::<Named>(&[0x01, 0x01]), a);

    let b = Named {
        d: Discr::D2,
        c: Content::B { n: 0x0102 },
    };
    assert_eq!(ser(&b), vec![0x02, 0x01, 0x02]);
    assert_eq!(de::<Named>(&[0x02, 0x01, 0x02]), b);
}

#[test]
fn tuple_member_is_addressed_by_index() {
    let v = Tuple::V(Discr::D1, Content::A { t: false });
    assert_eq!(ser(&v), vec![0x00, 0x01, 0x00]);
    assert_eq!(de::<Tuple>(&[0x00, 0x01, 0x00]), v);
}

#[test]
fn the_stored_discriminant_selects_the_variant() {
    // Discr::D2 selects `Content::B`, whose payload is two bytes. One byte left must be an error,
    // not a decode as `A { t: bool }`: that is what "the discriminant is honoured" means on the wire.
    let short =
        Named::tlspl_deserialize_from_complete(&mut btd(&[0x02u8, 0x01u8][..]).enter().unwrap());
    assert!(
        short.is_err(),
        "Discr::D2 selects B, a one-byte payload must not decode: {short:?}"
    );
}

// A discriminant whose source enum carries a value that needs the full repr width.

#[derive(Debug, Clone, Copy, PartialEq, thalassa::TlsplAll)]
#[repr(u16)]
enum WideDiscr {
    W1 = 0x0301,
}

#[derive(Debug, PartialEq, thalassa::TlsplAll)]
struct WideSelect {
    d: WideDiscr,
    #[tlspl(select = self.d)]
    c: WideContent,
}

#[derive(Debug, Clone, Copy, PartialEq, thalassa::TlsplAll)]
#[repr(u8)]
enum NarrowDiscr {
    N1 = 0x07,
}

#[derive(Debug, PartialEq, thalassa::TlsplAll)]
#[tlspl(untagged)]
#[repr(u16)]
enum WideContent {
    #[tlspl(discriminant = "NarrowDiscr::N1")]
    A { t: bool },
    #[tlspl(discriminant = "WideDiscr::W1")]
    B { t: bool },
}

#[derive(Debug, PartialEq, thalassa::TlsplAll)]
struct NarrowSelect {
    d: NarrowDiscr,
    #[tlspl(select = self.d)]
    c: WideContent,
}

#[test]
fn a_wide_source_discriminant_survives_a_matching_repr() {
    // 0x0301 is the value that used to be truncated to 0x01 when the target's repr was `u8`; against
    // a `repr(u16)` target it has to arrive intact. (Handing it to a `repr(u8)` target is a compile
    // error, pinned by `fail.select_source_wider_than_repr.rs`.)
    let bytes = [0x03u8, 0x01, 0x01];
    assert_eq!(
        WideSelect::tlspl_deserialize_from_complete(&mut btd(&bytes[..]).enter().unwrap())
            .expect("deserialize"),
        WideSelect {
            d: WideDiscr::W1,
            c: WideContent::B { t: true }
        }
    );
}

#[test]
fn a_narrower_source_is_fine() {
    // The other direction has to keep working: 0x07 fits in a `repr(u16)` untagged enum.
    let bytes = [0x07u8, 0x01];
    assert_eq!(
        NarrowSelect::tlspl_deserialize_from_complete(&mut btd(&bytes[..]).enter().unwrap())
            .expect("deserialize"),
        NarrowSelect {
            d: NarrowDiscr::N1,
            c: WideContent::A { t: true }
        }
    );
}
