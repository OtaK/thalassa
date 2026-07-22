#![allow(dead_code)]

use std::borrow::Cow;

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u8)]
enum Discrs {
    D1 = 0x01,
    D2 = 0x03,
    D3 = 0xFF,
}

#[derive(Debug, thalassa::TlsplSize, thalassa::TlsplSerialize)]
#[tlspl(untagged)]
#[repr(u8)]
enum Test<'a> {
    #[tlspl(discriminant = "Discrs::D1")]
    Variant1 { thing: bool },
    #[tlspl(discriminant = "Discrs::D2")]
    Variant2 { number: u64 },
    #[tlspl(discriminant = "Discrs::D3")]
    Variant3(Cow<'a, [u8]>),
}

fn main() {}
