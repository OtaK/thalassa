use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use mls_spec::{
    // credential::Credential,
    // defs::{Capabilities, CiphersuiteId, ExtensionType, ProposalType},
    // group::{
    //     ExternalSender,
    //     extensions::{Extension, RatchetTreeExtension},
    // },
    key_schedule::GroupContext,
    // tree::{
    //     TreeNode,
    //     leaf_node::{LeafNode, LeafNodeSource},
    // },
};
use std::{borrow::Cow, hint::black_box};

const GROUPCTX_SAMPLE: &'static [u8] = include_bytes!("sample.groupctx.tlspl");
const VLBYTES_SAMPLE: &'static [u8] = include_bytes!("sample.vlbytes_100KB.tlspl");

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

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

fn bench_tls_codec<T: tls_codec::Serialize + tls_codec::Deserialize>(
    sample_name: &str,
    sample: &[u8],
    c: &mut Criterion,
) {
    let mut group = c.benchmark_group(&format!("{sample_name}/tls_codec"));
    group.throughput(Throughput::Bytes(sample.len() as u64));

    group.bench_function("de", |b| {
        b.iter(|| black_box(T::tls_deserialize(black_box(&mut &sample[..])).unwrap()));
    });

    group.bench_function("ser", |b| {
        let value = T::tls_deserialize(&mut &sample[..]).unwrap();
        let capacity = value.tls_serialized_len();

        b.iter_batched_ref(
            || black_box(Vec::with_capacity(black_box(capacity))),
            |buf| black_box(value.tls_serialize(black_box(buf)).unwrap()),
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_thalassa<'a, T: thalassa::TlsplDeserialize<'a> + thalassa::TlsplSerialize + 'a>(
    sample_name: &str,
    sample: &'a [u8],
    c: &mut Criterion,
) {
    let mut group = c.benchmark_group(&format!("{sample_name}/thalassa"));
    group.throughput(Throughput::Bytes(sample.len() as u64));

    // FIXME: I get 500TB/s on this. There's absolutely no way that's correct so, uh, maybe find a way?
    group.bench_function("de", |b| {
        b.iter(|| black_box(T::tlspl_deserialize_from(black_box(&mut &sample[..])).unwrap()));
    });

    group.bench_function("ser", |b| {
        let value = T::tlspl_deserialize_from(&mut &sample[..]).unwrap();
        let capacity = value.tlspl_serialized_len();

        b.iter_batched_ref(
            || black_box(Vec::with_capacity(black_box(capacity))),
            |buf| black_box(value.tlspl_serialize_to(black_box(buf)).unwrap()),
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn perf(c: &mut Criterion) {
    // let mut test_gc = GroupContext::with_group_id(b"test".into());
    // test_gc.confirmed_transcript_hash = vec![222u8; 32].into();
    // test_gc.epoch = 80000000;
    // test_gc.tree_hash = vec![111u8; 32].into();
    // test_gc.cipher_suite = CiphersuiteId::new_unchecked(1);
    // test_gc.extensions.push(Extension::RequiredCapabilities(
    //     mls_spec::group::RequiredCapabilities {
    //         extension_types: vec![
    //             ExtensionType::new_unchecked(234),
    //             ExtensionType::new_unchecked(235),
    //         ],
    //         proposal_types: vec![
    //             ProposalType::new_unchecked(189),
    //             ProposalType::new_unchecked(190),
    //         ],
    //         credential_types: vec![],
    //     },
    // ));

    // test_gc
    //     .extensions
    //     .push(Extension::ExternalSenders(vec![ExternalSender {
    //         signature_key: vec![22; 32].into(),
    //         credential: Credential::basic(b"alice".into()),
    //     }]));

    // let tn = TreeNode::LeafNode(LeafNode {
    //     encryption_key: vec![1; 32].into(),
    //     signature_key: vec![1; 32].into(),
    //     credential: Credential::basic(b"alice".into()),
    //     capabilities: Capabilities {
    //         versions: Default::default(),
    //         ciphersuites: Default::default(),
    //         extensions: Default::default(),
    //         proposals: Default::default(),
    //         credentials: Default::default(),
    //     },
    //     source: LeafNodeSource::Commit {
    //         parent_hash: vec![9; 32].into(),
    //     },
    //     extensions: vec![Extension::ApplicationId(vec![u8::MAX; 1024])],
    //     signature: vec![24; 32].into(),
    // });

    // // Add 20k LeafNodes lol
    // test_gc
    //     .extensions
    //     .push(Extension::RatchetTree(RatchetTreeExtension {
    //         ratchet_tree: vec![Some(tn); 20_000].into(),
    //     }));

    // // get a sample of "known good" from tls_codec
    // let sample = test_gc.tls_serialize_detached().unwrap();
    // let thalassa_gc = black_box(
    //     ThalassaGroupContext::tlspl_deserialize_from(black_box(&mut &sample[..])).unwrap(),
    // );

    // assert_eq!(thalassa_gc.version as u16, test_gc.version as u16);
    // assert_eq!(thalassa_gc.cipher_suite, *test_gc.cipher_suite);
    // assert_eq!(&*thalassa_gc.group_id, test_gc.group_id());
    // assert_eq!(thalassa_gc.epoch, test_gc.epoch);
    // assert_eq!(&*thalassa_gc.tree_hash, test_gc.tree_hash.as_slice());
    // assert_eq!(
    //     &*thalassa_gc.confirmed_transcript_hash,
    //     test_gc.confirmed_transcript_hash.as_slice()
    // );

    bench_tls_codec::<GroupContext>("group_context", GROUPCTX_SAMPLE, c);
    bench_thalassa::<ThalassaGroupContext>("group_context", GROUPCTX_SAMPLE, c);

    bench_tls_codec::<tls_codec::VLBytes>("vlbytes_100kb", VLBYTES_SAMPLE, c);
    bench_thalassa::<Cow<[u8]>>("vlbytes_100kb", VLBYTES_SAMPLE, c);
}

criterion_group!(benches, perf);
criterion_main!(benches);
