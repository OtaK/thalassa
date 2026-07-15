use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use mls_spec2::{
    credential::Credential,
    defs::{Capabilities, CiphersuiteId, ExtensionType, ProposalType},
    group::{
        ExternalSender,
        extensions::{Extension, RatchetTreeExtension},
    },
    key_schedule::GroupContext,
    tree::{
        TreeNode,
        leaf_node::{LeafNode, LeafNodeSource},
    },
};
use std::{borrow::Cow, hint::black_box};
use thalassa::{TlsplDeserialize as _, TlsplSerialize};
use tls_codec::Serialize as _;

fn random_vec_with_len(len: usize) -> Vec<u8> {
    let mut vec = vec![0u8; len];
    rand::fill(&mut vec);
    vec
}

fn generate_vlbytes() -> Vec<u8> {
    random_vec_with_len(100_000).tlspl_serialize().unwrap()
}

fn generate_groupctx() -> Vec<u8> {
    let mut test_gc = GroupContext::with_group_id(b"test".into());
    test_gc.confirmed_transcript_hash = random_vec_with_len(32).into();
    test_gc.epoch = rand::random();
    test_gc.tree_hash = random_vec_with_len(32).into();
    test_gc.cipher_suite = CiphersuiteId::new_unchecked(rand::random());
    test_gc.extensions.push(Extension::RequiredCapabilities(
        mls_spec2::group::RequiredCapabilities {
            extension_types: vec![
                ExtensionType::new_unchecked(rand::random()),
                ExtensionType::new_unchecked(rand::random()),
            ],
            proposal_types: vec![
                ProposalType::new_unchecked(rand::random()),
                ProposalType::new_unchecked(rand::random()),
            ],
            credential_types: vec![],
        },
    ));

    test_gc
        .extensions
        .push(Extension::ExternalSenders(vec![ExternalSender {
            signature_key: random_vec_with_len(32).into(),
            credential: Credential::basic(b"alice".into()),
        }]));

    let ratchet_tree = (0usize..1000)
        .into_iter()
        .map(|i| {
            Some(TreeNode::LeafNode(LeafNode {
                encryption_key: random_vec_with_len(32).into(),
                signature_key: random_vec_with_len(32).into(),
                credential: Credential::basic(format!("alice_{i}").into_bytes().into()),
                capabilities: Capabilities {
                    versions: Default::default(),
                    ciphersuites: Default::default(),
                    extensions: Default::default(),
                    proposals: Default::default(),
                    credentials: Default::default(),
                },
                source: LeafNodeSource::Commit {
                    parent_hash: random_vec_with_len(32).into(),
                },
                extensions: vec![Extension::ApplicationId(random_vec_with_len(1024))],
                signature: random_vec_with_len(32).into(),
            }))
        })
        .collect::<Vec<_>>();

    // Add 20k LeafNodes lol
    test_gc
        .extensions
        .push(Extension::RatchetTree(RatchetTreeExtension {
            ratchet_tree: ratchet_tree.into(),
        }));

    // get a sample of "known good" from tls_codec
    test_gc.tls_serialize_detached().unwrap()
}

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
    sample_generator: fn() -> Vec<u8>,
    c: &mut Criterion,
) {
    let sample = sample_generator();
    let mut group = c.benchmark_group(&format!("{sample_name}({})/tls_codec", sample.len()));
    group.throughput(Throughput::Bytes(sample.len() as u64));

    group.bench_function("de", |b| {
        b.iter_batched(
            || black_box(sample_generator()),
            |sample| black_box(T::tls_deserialize(black_box(&mut &sample[..])).unwrap()),
            BatchSize::SmallInput,
        );
    });

    group.bench_function("ser", |b| {
        let value = T::tls_deserialize(&mut &sample_generator()[..]).unwrap();
        let capacity = value.tls_serialized_len();

        b.iter_batched_ref(
            || black_box(Vec::with_capacity(black_box(capacity))),
            |buf| black_box(value.tls_serialize(black_box(buf)).unwrap()),
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_thalassa_vlbytes(c: &mut Criterion) {
    let value_len = generate_vlbytes().len();
    let mut group = c.benchmark_group(&format!("vlbytes({value_len})/thalassa"));
    group.throughput(Throughput::Bytes(value_len as u64));

    group.bench_function("de", |b| {
        b.iter_batched(
            || generate_vlbytes(),
            |sample| {
                let value = black_box(
                    Cow::<[u8]>::tlspl_deserialize_from(black_box(&mut &sample[..])).unwrap(),
                );

                black_box(value[0])
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("ser", |b| {
        let sample = generate_vlbytes();
        let value = Cow::<[u8]>::tlspl_deserialize_from(&mut &sample[..]).unwrap();
        b.iter_batched_ref(
            || Vec::with_capacity(black_box(value_len)),
            |buf| black_box(value.tlspl_serialize_to(black_box(buf)).unwrap()),
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_thalassa_groupctx(c: &mut Criterion) {
    let value_len = generate_groupctx().len();
    let mut group = c.benchmark_group(&format!("group_context({value_len})/thalassa"));
    group.throughput(Throughput::Bytes(value_len as u64));

    group.bench_function("de", |b| {
        b.iter_batched(
            || generate_groupctx(),
            |sample| {
                let value = black_box(
                    ThalassaGroupContext::tlspl_deserialize_from(black_box(&mut &sample[..]))
                        .unwrap(),
                );

                black_box(value.cipher_suite)
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("ser", |b| {
        b.iter_batched_ref(
            || {
                (
                    generate_groupctx(),
                    Vec::with_capacity(black_box(value_len)),
                )
            },
            |(value, buf)| black_box(value.tlspl_serialize_to(black_box(buf)).unwrap()),
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn perf(c: &mut Criterion) {
    bench_tls_codec::<GroupContext>("group_context", generate_groupctx, c);
    bench_thalassa_groupctx(c);

    bench_tls_codec::<tls_codec::VLBytes>("vlbytes", generate_vlbytes, c);
    bench_thalassa_vlbytes(c);
}

criterion_group!(benches, perf);
criterion_main!(benches);
