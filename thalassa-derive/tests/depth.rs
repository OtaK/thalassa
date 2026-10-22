//! Depth accounting for *derived* types.
//!
//! A struct or enum is one node: its members are read on its own level, so deriving adds no depth
//! level. A container hop does open one — which is what turns a recursive derived type from a
//! stack overflow into an error.
use std::cell::Cell;

use thalassa::error::{TlsplReadError, TlsplReadResult, TlsplWriteResult};
use thalassa::io::{DepthAwareReader, DepthAwareReaderGuard, Read, ReadError, Write};
use thalassa::{TlsplDeserialize, TlsplSerialize, TlsplSize};

/// The limit a plain [`DepthAwareReader`] entry point runs with.
const LIMIT: u16 = <DepthAwareReader<&'static [u8]>>::DEFAULT_LIMIT;

thread_local! {
    static SEEN_DEPTH: Cell<u16> = const { Cell::new(0) };
}

/// Records the depth it was read at, so a type's own tick can be observed from the outside.
/// One wire byte.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct Probe;

impl TlsplSize for Probe {
    fn tlspl_serialized_len(&self) -> usize {
        1
    }
}

impl TlsplSerialize for Probe {
    fn tlspl_serialize_to<W: Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        writer.write_all(&[0x00])?;
        Ok(1)
    }
}

impl<'tlspl> TlsplDeserialize<'tlspl> for Probe {
    fn tlspl_deserialize_from<R: Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, R>,
    ) -> TlsplReadResult<Self> {
        reader.read_byte()?;
        SEEN_DEPTH.with(|depth| depth.set(reader.depth()));
        Ok(Probe)
    }
}

/// The depth `T`'s fields were read at, or 0 if none were.
fn first_seen_depth<'tlspl, T: TlsplDeserialize<'tlspl> + 'tlspl>(bytes: &'tlspl [u8]) -> u16 {
    SEEN_DEPTH.with(|depth| depth.set(0));
    parse::<T>(bytes).expect("deserialize");
    SEEN_DEPTH.with(Cell::get)
}

fn parse<'tlspl, T: TlsplDeserialize<'tlspl> + 'tlspl>(bytes: &'tlspl [u8]) -> TlsplReadResult<T> {
    let mut reader = DepthAwareReader::from_reader(bytes);
    let mut guard = reader.enter()?;
    T::tlspl_deserialize_from(&mut guard)
}

#[derive(Debug, thalassa::TlsplAll)]
struct DerivedStruct {
    probe: Probe,
}

#[derive(Debug, thalassa::TlsplAll)]
#[repr(u8)]
enum DerivedEnum {
    Only(Probe),
}

#[derive(Debug, PartialEq, thalassa::TlsplAll)]
struct Node {
    kids: Vec<Node>,
}

fn chain(depth: u16) -> Node {
    let mut node = Node { kids: Vec::new() };
    for _ in 0..depth {
        node = Node { kids: vec![node] };
    }
    node
}

#[test]
fn derived_struct_stays_on_its_parent_level() {
    assert_eq!(
        first_seen_depth::<DerivedStruct>(&[0x00]),
        1,
        "a struct is one node: its fields are read on its own level"
    );
}

#[test]
fn derived_enum_stays_on_its_parent_level() {
    assert_eq!(
        first_seen_depth::<DerivedEnum>(&[0x00, 0x00]),
        1,
        "an enum is one node: its variant's fields are read on its own level"
    );
}

#[test]
fn recursive_derived_type_is_refused_by_the_depth_limit() {
    // Legal depth still round-trips…
    let legal = chain(LIMIT / 2);
    let bytes = legal.tlspl_serialize().expect("serialize");
    assert_eq!(
        parse::<Node>(&bytes).expect("a chain this deep parses"),
        legal
    );

    // …and past the limit the container hop refuses, instead of recursing into the stack. 257
    // frames would not overflow a test thread, so a missing tick shows up as `Ok` here.
    let too_deep = chain(LIMIT + 1);
    let bytes = too_deep.tlspl_serialize().expect("serialize");
    let err = parse::<Node>(&bytes).expect_err("depth limit must fire");
    assert!(
        matches!(
            err,
            TlsplReadError::Parsio(ReadError::AllowedDepthOverflow { .. })
        ),
        "expected AllowedDepthOverflow, got {err:?}"
    );
}
