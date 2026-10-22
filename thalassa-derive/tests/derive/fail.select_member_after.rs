#![allow(dead_code)]

//! The discriminant has to be read before the member that consumes it.

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
struct S {
    #[tlspl(select = self.later)]
    e: E,
    later: D,
}

fn main() {}
