#![allow(dead_code)]

//! `self.<member>` has to name a member that exists.

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
    d: D,
    #[tlspl(select = self.missing)]
    e: E,
}

fn main() {}
