#![allow(dead_code)]
use std::borrow::Cow;

mod inception {
    pub mod inner {
        pub mod deeper {
            #[derive(Debug, Clone, Copy, thalassa::TlsplAll)]
            #[repr(transparent)]
            pub struct DeepSeaCreature(pub bool);
        }

        #[derive(Debug, Clone, Copy, thalassa::TlsplAll)]
        #[repr(u8)]
        pub enum Discrs {
            D1 = 0x01,
            D2 = 0x03,
            D3 = 0xFF,
        }

        #[derive(Debug, thalassa::TlsplAll)]
        #[tlspl(untagged)]
        #[repr(u8)]
        pub enum Test<'a> {
            #[tlspl(discriminant = "Discrs::D1")]
            Variant1 { thing: bool },
            #[tlspl(discriminant = "Discrs::D2")]
            Variant2 { number: u64 },
            #[tlspl(discriminant = "Discrs::D3")]
            Variant3(std::borrow::Cow<'a, [u8]>),
        }
    }
}

#[derive(Debug, thalassa::TlsplAll)]
struct SelectedStruct<'a> {
    pub top_dog: inception::inner::Discrs,
    pub deep: inception::inner::deeper::DeepSeaCreature,
    pub unrelated_field: Cow<'a, [u8]>,
    pub another_unrelated_field: u64,
    #[tlspl(select = top_dog)]
    pub wingardium_selecta: inception::inner::Test<'a>,
}

#[derive(Debug, thalassa::TlsplAll)]
struct Intermediate {
    pub inner: IntermediateInner,
}

#[derive(Debug, thalassa::TlsplAll)]
struct IntermediateInner {
    pub target: inception::inner::Discrs,
}
#[derive(Debug, thalassa::TlsplAll)]
struct DeepSelectStruct<'a> {
    pub deep_target: Intermediate,
    pub deep: inception::inner::deeper::DeepSeaCreature,
    pub unrelated_field: Cow<'a, [u8]>,
    pub another_unrelated_field: u64,
    #[tlspl(select = deep_target.inner.target)]
    pub wingardium_selecta: inception::inner::Test<'a>,
}

fn main() {}
