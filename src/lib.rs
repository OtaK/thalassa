#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

use crate::error::{TlsplReadResult, TlsplWriteResult};

pub mod error;
pub mod types;

pub use parsio as io;

use parsio::{Read, Write};
#[cfg(feature = "derive")]
pub use thalassa_derive::*;

pub trait TlsplSize {
    fn tlspl_serialized_len(&self) -> usize;
}

// Blanket impl
impl<T: TlsplSize> TlsplSize for &T {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        (*self).tlspl_serialized_len()
    }
}

pub trait TlsplDeserialize<'tlspl>: TlsplSize {
    fn tlspl_deserialize_from<R: Read<'tlspl>>(reader: &mut R) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl;
}

pub trait TlsplSerialize: TlsplSize {
    fn tlspl_serialize_to<W: Write>(&self, writer: &mut W) -> TlsplWriteResult<usize>;
    #[inline]
    fn tlspl_serialize(&self) -> TlsplWriteResult<Vec<u8>> {
        let len = self.tlspl_serialized_len();
        let mut buf = Vec::with_capacity(len);
        let written = self.tlspl_serialize_to(&mut buf)?;
        debug_assert_eq!(written, len, "len != written, something is awry");
        Ok(buf)
    }
}

// Blanket impl
impl<T: TlsplSerialize> TlsplSerialize for &T {
    #[inline]
    fn tlspl_serialize_to<W: Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        (*self).tlspl_serialize_to(writer)
    }
}
