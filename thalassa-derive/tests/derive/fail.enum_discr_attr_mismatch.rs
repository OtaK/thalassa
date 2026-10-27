#![allow(dead_code)]

const SOME_CONST: u8 = 5;

#[derive(thalassa::TlsplAll)]
#[repr(u8)]
enum Mismatched {
    #[tlspl(discriminant = 7)]
    Variant1 { thing: bool } = SOME_CONST,
}

fn main() {}
