#![allow(dead_code)]

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

fn main() {}
