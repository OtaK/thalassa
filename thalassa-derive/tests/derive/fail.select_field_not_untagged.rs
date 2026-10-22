#![allow(dead_code)]

//! A `#[tlspl(select = ...)]` member must be a `#[tlspl(untagged)]` enum: a wrapper is refused.

#[derive(Debug, Clone, Copy, thalassa::TlsplAll)]
#[repr(u8)]
enum D {
    D1 = 1,
}

#[derive(Debug, thalassa::TlsplAll)]
#[tlspl(untagged)]
#[repr(u8)]
enum E {
    #[tlspl(discriminant = "D::D1")]
    A { t: bool },
}

#[derive(Debug, thalassa::TlsplAll)]
struct Wrapped {
    d: D,
    #[tlspl(select = self.d)]
    e: Vec<E>,
}

fn main() {}
