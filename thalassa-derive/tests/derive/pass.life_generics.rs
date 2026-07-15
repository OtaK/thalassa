#![allow(dead_code)]

use std::borrow::Cow;
use thalassa_derive::{TlsplDeserialize, TlsplSerialize, TlsplSize};

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
struct LifeGeneric<'a, T> {
    pub generic: T,
    pub other: Cow<'a, [u8]>,
    pub otherb: Cow<'a, str>,
}

fn main() {}
