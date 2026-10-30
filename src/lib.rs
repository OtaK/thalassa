#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

use crate::error::{TlsplReadResult, TlsplWriteResult};

#[cfg(feature = "futures-codec")]
#[cfg_attr(docsrs, doc(cfg(feature = "futures-codec")))]
pub mod codec;
pub mod error;
pub mod types;
pub mod util;

pub use parsio as io;

use parsio::{DepthAwareReaderGuard, Read, Write};

#[cfg(feature = "derive")]
pub use thalassa_derive::*;

/// Hinting trait that is essential: returns the byte-size of the TLSPL struct when serialized
#[diagnostic::on_unimplemented(
    message = "`{Self}` doesn't implement `TlsplSize` or is not a TLSPL type",
    note = r#"not every Rust type has one:
    - `u8` deliberately has no impls; write a uint8 field literally as `u8` (the derive enables it),
      and keep it a literal: an alias (`type B = u8;`, `use core::primitive::u8 as B;`) wouldn't work.
    - Floating-points (`f32`/`f64`) have no representation in TLSPL
    - `usize` and `isize` are not wire-stable as it depends on your target's pointer width, so no dice either
    - signed integers have no representation in TLSPL
    - Btw: use `Cow<[u8]>`/`Vec<u8>`/`&[u8]`/`String`/`&str` for VLBytes."#
)]
pub trait TlsplSize {
    fn tlspl_serialized_len(&self) -> usize;
}

/// Trait that allows deserializing a TLSPL struct from its wire repr
#[diagnostic::on_unimplemented(
    message = "`{Self}` doesn't implement `TlsplDeserialize` or is not a TLSPL type",
    note = r#"not every Rust type has one:
    - `u8` deliberately has no impls; write a uint8 field literally as `u8` (the derive enables it),
      and keep it a literal: an alias (`type B = u8;`, `use core::primitive::u8 as B;`) wouldn't work.
    - Floating-points (`f32`/`f64`) have no representation in TLSPL
    - `usize` and `isize` are not wire-stable as it depends on your target's pointer width, so no dice either
    - signed integers have no representation in TLSPL
    - Btw: use `Cow<[u8]>`/`Vec<u8>`/`&[u8]`/`String`/`&str` for VLBytes."#
)]
pub trait TlsplDeserialize<'tlspl>: TlsplSize {
    /// Note: To acquire a `DepthAwareReaderGuard`, you must wrap your reader (bytes/std/etc) in
    /// `thalassa::io::DepthAwareReader::from_reader(my_reader)` then call `.enter().map_err(TlsplReadError::from)?` on it.
    ///
    /// You can also customize the recursion limit with `DepthAwareReader::from_reader_with_limit(my_reader, limit)`.
    fn tlspl_deserialize_from<R: Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, R>,
    ) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl;

    /// "complete" version of [`Self::tlspl_deserialize_from`], that errors out if the buffer hasn't been entirely consumed.
    fn tlspl_deserialize_from_complete<R: Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, R>,
    ) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        let value = Self::tlspl_deserialize_from(reader)?;
        match reader.remaining() {
            Some(0) => Ok(value),
            None if reader.is_eof()? => Ok(value),
            Some(extra) => Err(crate::error::TlsplReadError::TrailingBytes { extra }),
            None => {
                Err(crate::error::TlsplReadError::TrailingBytes {
                    extra: usize::MAX, // count unknown
                })
            }
        }
    }
}

/// Trait that allows to serialize TLSPL structures to the wire
///
/// Note that no depth-tracking is done on serialization.
#[diagnostic::on_unimplemented(
    message = "`{Self}` doesn't implement `TlsplSerialize` or is not a TLSPL type",
    note = r#"not every Rust type has one:
        - `u8` deliberately has no impls; write a uint8 field literally as `u8` (the derive enables it),
            and keep it a literal: an alias (`type B = u8;`, `use core::primitive::u8 as B;`) wouldn't work.
        - Floating-points (`f32`/`f64`) have no representation in TLSPL
        - `usize` and `isize` are not wire-stable as it depends on your target's pointer width, so no dice either
        - signed integers have no representation in TLSPL
        - Btw: use `Cow<[u8]>`/`Vec<u8>`/`&[u8]`/`String`/`&str` for VLBytes."#
)]
pub trait TlsplSerialize: TlsplSize {
    /// Serializes this struct to the provided writer.
    ///
    /// Note that no depth-tracking is done on serialization.
    fn tlspl_serialize_to<W: Write>(&self, writer: &mut W) -> TlsplWriteResult<usize>;

    /// Serializes this struct to a newly allocated buffer.
    ///
    /// Note that no depth-tracking is done on serialization.
    #[inline]
    fn tlspl_serialize(&self) -> TlsplWriteResult<Vec<u8>> {
        let len = self.tlspl_serialized_len();
        let mut buf = Vec::with_capacity(len);
        let written = self.tlspl_serialize_to(&mut buf)?;
        debug_assert_eq!(written, len, "len != written, something is awry");
        Ok(buf)
    }
}

// Blanket impls
impl<T: TlsplSize> TlsplSize for &T {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        (*self).tlspl_serialized_len()
    }
}

impl<T: TlsplSize> TlsplSize for Box<T> {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        self.as_ref().tlspl_serialized_len()
    }
}

impl<'tlspl, T: TlsplDeserialize<'tlspl>> TlsplDeserialize<'tlspl> for Box<T> {
    #[inline]
    fn tlspl_deserialize_from<R: Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, R>,
    ) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        T::tlspl_deserialize_from(&mut reader.enter()?).map(Box::new)
    }
}

impl<T: TlsplSerialize> TlsplSerialize for &T {
    #[inline]
    fn tlspl_serialize_to<W: Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        (*self).tlspl_serialize_to(writer)
    }
}

impl<T: TlsplSerialize> TlsplSerialize for Box<T> {
    #[inline]
    fn tlspl_serialize_to<W: Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        self.as_ref().tlspl_serialize_to(writer)
    }
}

#[cfg(test)]
mod tests {
    mod regression {
        use std::borrow::Cow;
        use std::cell::Cell;
        use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

        use parsio::{DepthAwareReader, DepthAwareReaderGuard, ReadError};

        use super::super::{
            Read, TlsplDeserialize, TlsplSerialize, TlsplSize, TlsplWriteResult, Write,
        };
        use crate::error::{TlsplReadError, TlsplReadResult};

        /// The limit a plain [`DepthAwareReader`] entry point runs with.
        const LIMIT: u16 = <DepthAwareReader<&'static [u8]>>::DEFAULT_LIMIT;
        /// Deep enough to be legal on every hop.
        const LEGAL_DEPTH: u16 = LIMIT / 2;
        /// Past the limit, but shallow enough that a *missing* depth tick shows up as `Ok`
        /// instead of a stack overflow: the test must fail, not abort the test process.
        const TOO_DEEP: u16 = LIMIT + 1;

        /// Recursion through a container hop: `Vec<T>` ticks depth once per level.
        #[derive(Debug, PartialEq)]
        struct Node {
            kids: Vec<Node>,
        }

        impl TlsplSize for Node {
            fn tlspl_serialized_len(&self) -> usize {
                self.kids.tlspl_serialized_len()
            }
        }

        impl TlsplSerialize for Node {
            fn tlspl_serialize_to<W: Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
                self.kids.tlspl_serialize_to(writer)
            }
        }

        impl<'tlspl> TlsplDeserialize<'tlspl> for Node {
            fn tlspl_deserialize_from<R: Read<'tlspl>>(
                reader: &mut DepthAwareReaderGuard<'_, R>,
            ) -> TlsplReadResult<Self> {
                Ok(Self {
                    kids: Vec::<Node>::tlspl_deserialize_from(reader)?,
                })
            }
        }

        /// Recursion through `Box` alone: no container in between, so the `Box` hop carries the
        /// only depth tick on this path. One discriminant byte per level.
        #[derive(Debug, PartialEq)]
        enum Chain {
            Leaf,
            Next(Box<Chain>),
        }

        impl TlsplSize for Chain {
            fn tlspl_serialized_len(&self) -> usize {
                match self {
                    Chain::Leaf => 1,
                    Chain::Next(inner) => 1 + inner.tlspl_serialized_len(),
                }
            }
        }

        impl<'tlspl> TlsplDeserialize<'tlspl> for Chain {
            fn tlspl_deserialize_from<R: Read<'tlspl>>(
                reader: &mut DepthAwareReaderGuard<'_, R>,
            ) -> TlsplReadResult<Self> {
                match reader.read_byte()? {
                    0x00 => Ok(Chain::Leaf),
                    _ => Ok(Chain::Next(Box::tlspl_deserialize_from(reader)?)),
                }
            }
        }

        fn parse<'tlspl, T: TlsplDeserialize<'tlspl> + 'tlspl>(
            bytes: &'tlspl [u8],
        ) -> TlsplReadResult<T> {
            let mut reader = DepthAwareReader::from_reader(bytes);
            let mut guard = reader.enter()?;
            T::tlspl_deserialize_from(&mut guard)
        }

        fn node_chain(depth: u16) -> Node {
            let mut node = Node { kids: Vec::new() };
            for _ in 0..depth {
                node = Node { kids: vec![node] };
            }
            node
        }

        fn box_chain(depth: u16) -> Vec<u8> {
            let mut bytes = vec![0x01u8; depth as usize];
            bytes.push(0x00);
            bytes
        }

        fn assert_depth_overflow(err: TlsplReadError) {
            match err {
                TlsplReadError::Parsio(ReadError::AllowedDepthOverflow { depth, limit }) => {
                    assert_eq!(
                        limit, LIMIT,
                        "the default recursion limit must be in effect"
                    );
                    assert_eq!(
                        depth,
                        limit + 1,
                        "the first level past the limit is what fails"
                    );
                }
                other => panic!("expected AllowedDepthOverflow, got {other:?}"),
            }
        }

        #[test]
        fn deep_recursion_is_refused_instead_of_aborting() {
            // Container hop: `Vec<Node>`.
            let legal = node_chain(LEGAL_DEPTH);
            let bytes = legal.tlspl_serialize().expect("serialize");
            assert_eq!(
                parse::<Node>(&bytes).expect("a chain this deep parses"),
                legal
            );
            let too_deep = node_chain(TOO_DEEP).tlspl_serialize().expect("serialize");
            assert_depth_overflow(parse::<Node>(&too_deep).expect_err("depth limit must fire"));

            // Indirection hop: `Box<Chain>`, with nothing else in between.
            let too_deep = box_chain(TOO_DEEP);
            assert_depth_overflow(parse::<Chain>(&too_deep).expect_err("depth limit must fire"));
            let legal = box_chain(LEGAL_DEPTH);
            parse::<Chain>(&legal).expect("a chain this deep parses");
        }

        /// Records the depth it was read at, so a collection's own tick can be observed from the
        /// outside. One wire byte, and `Hash`/`Ord` so the sets and maps accept it.
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        struct DepthProbe;

        thread_local! {
            static SEEN_DEPTH: Cell<u16> = const { Cell::new(0) };
        }

        impl TlsplSize for DepthProbe {
            fn tlspl_serialized_len(&self) -> usize {
                1
            }
        }

        impl TlsplSerialize for DepthProbe {
            fn tlspl_serialize_to<W: Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
                writer.write_all(&[0x00])?;
                Ok(1)
            }
        }

        impl<'tlspl> TlsplDeserialize<'tlspl> for DepthProbe {
            fn tlspl_deserialize_from<R: Read<'tlspl>>(
                reader: &mut DepthAwareReaderGuard<'_, R>,
            ) -> TlsplReadResult<Self> {
                reader.read_byte()?;
                SEEN_DEPTH.with(|depth| depth.set(reader.depth()));
                Ok(DepthProbe)
            }
        }

        /// The depth `T`'s elements were read at, or 0 if none were.
        fn element_depth<'tlspl, T: TlsplDeserialize<'tlspl> + 'tlspl>(bytes: &'tlspl [u8]) -> u16 {
            SEEN_DEPTH.with(|depth| depth.set(0));
            let mut reader = DepthAwareReader::from_reader(bytes);
            let mut guard = reader.enter().expect("enter");
            T::tlspl_deserialize_from(&mut guard).expect("deserialize");
            SEEN_DEPTH.with(Cell::get)
        }

        /// Serializes `$value`, reads it back as `$ty`, and asserts its elements were read one
        /// level deeper than the entry point: a missing or doubled tick fails here.
        macro_rules! assert_ticks_once {
            ($ty:ty, $value:expr) => {{
                let value: $ty = $value;
                let bytes = value.tlspl_serialize().expect("serialize");
                assert_eq!(
                    element_depth::<$ty>(&bytes),
                    2,
                    "{} must tick exactly one depth level around its elements",
                    stringify!($ty)
                );
            }};
        }

        /// Every hop ticks depth exactly once per level, whatever it holds.
        ///
        /// Tuples are deliberately absent: their members are siblings in the topology, not each
        /// other's children, so a tuple stays on its parent's level.
        #[test]
        fn every_collection_hop_ticks_exactly_once() {
            // Baseline: a bare element is read at depth 1, the entry point's own level.
            assert_eq!(element_depth::<DepthProbe>(&[0x00]), 1);

            // Tuples are not hops either: their members are siblings in the topology, so they
            // stay on their parent's level.
            let bytes = (DepthProbe, DepthProbe)
                .tlspl_serialize()
                .expect("serialize");
            assert_eq!(
                element_depth::<(DepthProbe, DepthProbe)>(&bytes),
                1,
                "a tuple must not open a depth level"
            );

            assert_ticks_once!(Vec<DepthProbe>, vec![DepthProbe]);
            assert_ticks_once!(
                Cow<'_, [DepthProbe]>,
                Cow::<[DepthProbe]>::Owned(vec![DepthProbe])
            );
            assert_ticks_once!(HashSet<DepthProbe>, HashSet::from([DepthProbe]));
            assert_ticks_once!(BTreeSet<DepthProbe>, BTreeSet::from([DepthProbe]));
            assert_ticks_once!(
                HashMap<DepthProbe, DepthProbe>,
                HashMap::from([(DepthProbe, DepthProbe)])
            );
            assert_ticks_once!(
                BTreeMap<DepthProbe, DepthProbe>,
                BTreeMap::from([(DepthProbe, DepthProbe)])
            );
            assert_ticks_once!(Option<DepthProbe>, Some(DepthProbe));
            assert_ticks_once!(Box<DepthProbe>, Box::new(DepthProbe));
        }
    }
}
