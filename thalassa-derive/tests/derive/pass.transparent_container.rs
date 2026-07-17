#![allow(dead_code)]

use std::borrow::Cow;
use thalassa_derive::{TlsplDeserialize, TlsplSerialize, TlsplSize};

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
#[repr(transparent)]
struct TransparentStruct<'a>(pub Cow<'a, [u8]>);

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
struct OpaqueStruct<'a>(Cow<'a, [u8]>);

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
struct SimpleNewType(u8);

fn main() {}
