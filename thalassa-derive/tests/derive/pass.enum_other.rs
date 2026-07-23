#![allow(dead_code)]

#[derive(
    Debug,
    thalassa_derive::TlsplSize,
    thalassa_derive::TlsplSerialize,
    thalassa_derive::TlsplDeserialize,
)]
#[repr(u8)]
enum Thing {
    CaseA = 0,
    CaseB = 1,
    #[tlspl(other)]
    Unknown(u8),
}

#[derive(
    Debug,
    thalassa_derive::TlsplSize,
    thalassa_derive::TlsplSerialize,
    thalassa_derive::TlsplDeserialize,
)]
#[tlspl(extensible)]
#[repr(u8)]
enum ThingWithData<'a> {
    #[tlspl(discriminant = "Thing::CaseA")]
    CaseA { name: std::borrow::Cow<'a, str> },
    #[tlspl(discriminant = "Thing::CaseB")]
    CaseB { flag: bool },
    #[tlspl(other)]
    Unknown(u8, std::borrow::Cow<'a, [u8]>),
}

fn main() {}
