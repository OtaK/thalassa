#![allow(dead_code)]

use std::borrow::Cow;
use thalassa_derive::{TlsplDeserialize, TlsplSerialize, TlsplSize};

pub const FOUR_HUNDRED: u64 = 400;

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
struct RandomStruct<'a> {
    thing: bool,
    whoah: u64,
    bim: Cow<'a, [u8]>,
}

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
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

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
#[repr(u64)]
enum UnitDiscriminantsEnum {
    Variant1 = 12,
    Variant2,
}

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
#[repr(u64)]
enum FieldedDiscriminantsEnum<'a> {
    #[tlspl(discriminant = b' ')]
    Variant0,
    Variant1 {
        test: bool,
    } = 69,
    Variant2 {
        thing: u32,
    },
    Variant3 {
        potato: Cow<'a, str>,
    } = 300,
    #[tlspl(discriminant = "FOUR_HUNDRED")]
    Variant4,
}

fn main() {}
