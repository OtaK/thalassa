use std::borrow::Cow;

#[derive(Debug, thalassa::TlsplSize)]
#[tlspl(untagged)]
#[repr(u8)]
enum Test1<'a> {
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
}

#[derive(Debug, thalassa::TlsplSize)]
#[tlspl(untagged)]
#[repr(u8)]
enum Test2<'a> {
    Variant1(bool),
    Variant2(u64),
    Variant3(Cow<'a, [u8]>),
}

#[derive(Debug, thalassa::TlsplSize)]
#[tlspl(untagged)]
#[repr(u8)]
enum Test3<'a> {
    #[tlspl(discriminant = 1)]
    Variant1(bool),
    #[tlspl(discriminant = 2)]
    Variant2(u64),
    #[tlspl(discriminant = 3)]
    Variant3(Cow<'a, [u8]>),
}

fn main() {}
