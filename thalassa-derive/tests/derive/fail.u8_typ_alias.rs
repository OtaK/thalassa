#![allow(dead_code)]

//! A derive sees tokens, not resolved types: an aliased `uint8` field cannot be detected,
//! so it falls through to the trait path, which has no `u8` impls by design, until specialization lands.
//! To remove once specialization lands and we can *finally* impl traits for u8 bona fide

use core::primitive::u8 as Byte;

#[derive(Debug, thalassa::TlsplAll)]
struct Aliased {
    a: Byte,
}

fn main() {}
