#![allow(dead_code)]

//! `self.<member>` names the member a `select` reads its discriminant from — `self.<index>` is how
//! a tuple variant member is addressed, since it has no field name.

#[derive(Debug, Clone, Copy, thalassa::TlsplAll)]
#[repr(u8)]
enum D {
    D1 = 1,
    D2 = 2,
}

#[derive(Debug, thalassa::TlsplAll)]
#[tlspl(untagged)]
#[repr(u8)]
enum E {
    #[tlspl(discriminant = "D::D1")]
    A { t: bool },
    #[tlspl(discriminant = "D::D2")]
    B { n: u16 },
}

#[derive(Debug, thalassa::TlsplAll)]
struct Named {
    d: D,
    #[tlspl(select = self.d)]
    e: E,
}

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u8)]
enum Tuple {
    V(D, #[tlspl(select = self.0)] E),
}

fn main() {}
