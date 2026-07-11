#![allow(dead_code)]

use std::borrow::Cow;

#[derive(Debug, thalassa::TlsplAll)]
struct NaiveStruct<'a> {
    // f1: u8,
    f2: u16,
    f3: u32,
    f4: u64,
    f5: i8,
    f6: i16,
    f7: i32,
    f8: i64,
    bytes_cow: Cow<'a, [u8]>,
}

#[derive(Debug, thalassa::TlsplAll)]
struct RandomStruct<'a> {
    thing: bool,
    whoah: u64,
    bim: Cow<'a, [u8]>,
}

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u8)]
enum NaiveEnum<'a> {
    #[tlspl(discriminant = 1)]
    Variant1 {
        thing: bool,
    },
    Variant2 {
        number: u64,
    },
    Variant3(Cow<'a, [u8]>),
    Variant4 {
        useless_field: (),
    },
    Variant5 {
        #[tlspl(skip)]
        potato: [u8; 16],
    },
    Variant6,
    Variant7(RandomStruct<'a>),
}

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u64)]
enum UnitDiscriminantsEnum {
    Variant1 = 12,
    Variant2,
}

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u64)]
enum FieldedDiscriminantsEnum<'a> {
    Variant1 { test: bool } = 69,
    Variant2 { thing: u32 },
    Variant3 { potato: Cow<'a, str> } = 300,
    Variant4,
}

fn main() {}
