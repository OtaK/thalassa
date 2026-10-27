use gungraun::{Callgrind, FlamegraphConfig, prelude::*};
use std::{borrow::Cow, hint::black_box};
use thalassa::TlsplDeserialize;
use thalassa::io::DepthAwareReader;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    thalassa::TlsplDeserialize,
    thalassa::TlsplSerialize,
    thalassa::TlsplSize,
)]
#[repr(u16)]
enum ProtocolVersion {
    Reserved = 0,
    #[default]
    Mls10 = 1,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    thalassa::TlsplDeserialize,
    thalassa::TlsplSerialize,
    thalassa::TlsplSize,
)]
struct ThalassaGroupContext<'a> {
    pub version: ProtocolVersion,
    pub cipher_suite: u16,
    pub group_id: Cow<'a, [u8]>,
    pub epoch: u64,
    pub tree_hash: Cow<'a, [u8]>,
    pub confirmed_transcript_hash: Cow<'a, [u8]>,
    pub extensions: Vec<(u16, Cow<'a, [u8]>)>,
}

const GC_BYTES: &[u8; 23_600_156] = include_bytes!("sample.groupctx.tlspl");
const VLB_100KB_BYTES: &[u8; 100_004] = include_bytes!("sample.vlbytes_100KB.tlspl");

fn btd(bytes: &[u8]) -> DepthAwareReader<&[u8]> {
    DepthAwareReader::from_reader(bytes)
}

#[library_benchmark]
fn de_groupcontext() -> ThalassaGroupContext<'static> {
    black_box(
        ThalassaGroupContext::tlspl_deserialize_from(black_box(
            &mut btd(&GC_BYTES[..]).enter().unwrap(),
        ))
        .unwrap(),
    )
}

#[library_benchmark]
fn de_vlbytes() -> Cow<'static, [u8]> {
    black_box(
        Cow::<[u8]>::tlspl_deserialize_from(black_box(
            &mut btd(&VLB_100KB_BYTES[..]).enter().unwrap(),
        ))
        .unwrap(),
    )
}

library_benchmark_group!(name = de_groupctx, benchmarks = [de_groupcontext]);
library_benchmark_group!(name = de_bytes, benchmarks = [de_vlbytes]);

main!(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default().flamegraph(FlamegraphConfig::default())),
    library_benchmark_groups = [de_groupctx, de_bytes]
);
