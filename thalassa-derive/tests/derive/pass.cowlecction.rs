#![allow(dead_code)]

use std::borrow::Cow;
use thalassa_derive::{TlsplDeserialize, TlsplSerialize, TlsplSize};

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
struct Cowllection<'a> {
    pub inner: Vec<Cow<'a, [u8]>>,
}

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
#[repr(transparent)]
struct OptionalCollection<'a>(Vec<Option<Cow<'a, [u8]>>>);

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
struct CowlectionCollection<'a> {
    col: Vec<Cowllection<'a>>,
}

fn main() {}
