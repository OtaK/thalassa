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
    fn tlspl_deserialize_from<R: Read<'tlspl>>(reader: &mut R) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        T::tlspl_deserialize_from(reader).map(Box::new)
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
