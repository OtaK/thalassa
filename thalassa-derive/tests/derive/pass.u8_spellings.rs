#![allow(dead_code, unused_parens)]

//! `u8` fields must work in the derive even if referenced in weird ways, eg
//! by using `core::primitive:u8` or `::std::primitive::u8`

#[derive(Debug, thalassa::TlsplAll)]
struct Plain {
    a: u8,
}

#[derive(Debug, thalassa::TlsplAll)]
struct Qualified {
    b: core::primitive::u8,
    c: ::std::primitive::u8,
}

#[derive(Debug, thalassa::TlsplAll)]
struct Parenthesized {
    d: (u8),
}

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u8)]
enum Members {
    Named { e: core::primitive::u8 },
    Tuple(u8),
}

/// on purpose, we cause the `syn::Group` wrapping
macro_rules! generated {
    ($name:ident, $t:ty) => {
        #[derive(Debug, thalassa::TlsplAll)]
        struct $name {
            f: $t,
        }
    };
}

generated!(FromMacro, u8);
generated!(FromMacroQualified, core::primitive::u8);

/// Test the same with `extensible` which does its own weird shenanigans
#[derive(Debug, thalassa::TlsplAll)]
#[tlspl(extensible)]
#[repr(u8)]
enum Ext {
    #[tlspl(discriminant = 0x11)]
    A { g: core::primitive::u8 },
    #[tlspl(discriminant = 0x12)]
    B((u8)),
}

fn main() {}
