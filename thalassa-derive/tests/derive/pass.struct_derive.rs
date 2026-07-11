#![allow(dead_code)]

use std::borrow::Cow;
use thalassa_derive::{TlsplDeserialize, TlsplSerialize, TlsplSize};

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
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

fn main() {}
