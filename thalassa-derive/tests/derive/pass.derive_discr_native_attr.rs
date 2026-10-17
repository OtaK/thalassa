#![allow(dead_code)]

const SOME_CONST: u8 = 5;
const OTHER_CONST: u8 = 9;

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u8)]
enum NativeExprPlusAttr {
    #[tlspl(discriminant = 5)]
    Variant1 {
        thing: bool,
    } = SOME_CONST,
    Variant2 {
        number: u64,
    },
}

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u8)]
enum NativeLitPlusAttr {
    #[tlspl(discriminant = 1)]
    Variant1 {
        thing: bool,
    } = 1,
    #[tlspl(discriminant = b' ')]
    Variant2,
    Variant3 {
        blob: Vec<u8>,
    } = 69,
    Variant4,
}

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u8)]
enum PathAttrPlusNative {
    #[tlspl(discriminant = SOME_CONST)]
    Variant1 { thing: bool } = SOME_CONST,
    #[tlspl(discriminant = OTHER_CONST)]
    Variant2 { number: u64 } = OTHER_CONST,
}

fn main() {}
