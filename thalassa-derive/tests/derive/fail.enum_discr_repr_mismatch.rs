#![allow(dead_code)]

const WIDE: u16 = 0x0301;

#[derive(thalassa::TlsplAll)]
#[repr(u8)]
enum Mismatched {
    #[tlspl(discriminant = WIDE)]
    Variant1 { thing: bool },
}

fn main() {}
