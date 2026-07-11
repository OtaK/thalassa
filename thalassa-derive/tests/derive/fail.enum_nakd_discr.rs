#[derive(thalassa::TlsplAll)]
#[repr(u8)]
enum Variant {
    #[tlspl(discriminant = 100)]
    V1,
    V2,
}

fn main() {}
