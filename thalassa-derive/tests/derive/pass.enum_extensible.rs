#![allow(dead_code)]

use std::borrow::Cow;
use thalassa_derive::{TlsplDeserialize, TlsplSerialize, TlsplSize};

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
struct RandomStruct<'a> {
    thing: bool,
    whoah: u64,
    bim: Cow<'a, [u8]>,
}

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
#[tlspl(extensible)]
#[repr(u16)]
enum NaiveEnum<'a> {
    #[tlspl(discriminant = 6)]
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
    #[tlspl(other)]
    Fallback(u16, Cow<'a, [u8]>),
}

fn main() {}
