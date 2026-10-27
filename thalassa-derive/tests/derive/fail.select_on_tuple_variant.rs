#![allow(dead_code)]

//! A tuple member is addressed as `self.<index>`; its generated binding name is not part of the
//! attribute's grammar.

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
#[repr(u8)]
enum Outer {
    V(D, #[tlspl(select = tuple0)] E),
}

fn main() {}
