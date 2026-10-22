#![allow(dead_code)]

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u8)]
enum Discr {
    D1 = 1,
    D2 = 2,
}

#[derive(Debug, thalassa::TlsplAll)]
#[tlspl(extensible)]
#[repr(u8)]
enum Msg {
    #[tlspl(discriminant = "Discr::D1")]
    Named { t: u8 },
    #[tlspl(discriminant = "Discr::D2")]
    Tuple(u8),
}

#[derive(Debug, thalassa::TlsplAll)]
struct Holder {
    t: u8,
    m: Msg,
}

fn main() {}
