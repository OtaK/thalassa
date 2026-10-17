#![allow(dead_code)]

#[repr(u16)]
enum Source {
    S1 = 0x0301,
}

#[derive(thalassa::TlsplAll)]
#[repr(u8)]
enum Narrow {
    #[tlspl(discriminant = "Source::S1")]
    Variant1 { thing: bool },
}

fn main() {}
