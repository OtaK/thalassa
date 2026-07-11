#[derive(thalassa::TlsplAll)]
#[repr(u8)]
enum DiscrValue {
    V1 = 0xFE,
    V2 = 0xFF,
}

#[derive(thalassa::TlsplAll)]
#[repr(u8)]
enum Variant {
    #[tlspl(discriminant = "DiscrValue::V1")]
    V1 {
        thing: bool,
    },
    V2 {
        test: u64,
    },
}

fn main() {}
