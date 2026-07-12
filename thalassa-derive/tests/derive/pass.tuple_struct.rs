#![allow(dead_code)]

use thalassa_derive::{TlsplDeserialize, TlsplSerialize, TlsplSize};

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
struct FiveFieldTupleStruct(u32, u64, i16, bool, Option<()>);

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
struct TwoFieldTupleStruct(u32, u64);

#[derive(Debug, TlsplDeserialize, TlsplSerialize, TlsplSize)]
#[repr(transparent)]
struct OneFieldTupleStruct(u32);

fn main() {}
