use std::marker::PhantomData;

use crate::{
    TlsplDeserialize, TlsplSerialize, TlsplSize,
    io::{Read, Write},
};

mod bytes;
mod collections;
mod integer;
mod optional;
mod strings;

pub use integer::u24;

// TODO: MLS limits question mark?
// Right now this uses 2^62-1 on 64 bits (which is the normal TLSPL limits)
// and 2^30-1 on 32 bits (which is the MLS limits)
const MAX_LEN: usize = (usize::MAX >> 2) - 1;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ContentLengthLength {
    #[default]
    Empty = 0,
    Uint8 = 1,
    Uint16 = 2,
    Uint32 = 4,
    Uint64 = 8,
}

impl ContentLengthLength {
    #[inline]
    pub(crate) fn from_content_len(cl: usize) -> Self {
        if cl > MAX_LEN {
            return Self::Empty;
        }

        if cl < 0x40 {
            Self::Uint8
        } else if cl < 0x4000 {
            Self::Uint16
        } else if cl < 0x4000_0000 {
            Self::Uint32
        } else {
            Self::Uint64
        }
    }

    #[inline]
    pub(crate) fn write_content_len<W: Write>(
        &self,
        cl: usize,
        writer: &mut W,
    ) -> crate::error::TlsplWriteResult<usize> {
        debug_assert_eq!(*self, Self::from_content_len(cl), "Internal API misuse");

        let result = match self {
            ContentLengthLength::Empty => Ok(0),
            ContentLengthLength::Uint8 => writer.write(&[cl as u8]),
            ContentLengthLength::Uint16 => {
                writer.write(&((0x40_u16 << 8) | (cl as u16)).to_be_bytes())
            }
            ContentLengthLength::Uint32 => {
                writer.write(&((0x80_u32 << 24) | (cl as u32)).to_be_bytes())
            }
            ContentLengthLength::Uint64 => {
                writer.write(&((0xc0_u64 << 56) | (cl as u64)).to_be_bytes())
            }
        }
        .map_err(Into::into);

        // Sanity check for debug builds
        #[cfg(debug_assertions)]
        if let Ok(written) = &result {
            assert_eq!(*written as u8, *self as u8, "Write mismatch");
        }

        result
    }

    #[inline]
    pub(crate) fn read_content_len<'a, R: Read<'a>>(
        reader: &mut R,
    ) -> crate::error::TlsplReadResult<(Self, usize)> {
        let lb = reader.read_byte()?;
        let len = lb & 0x3F;
        let cll_mult = lb >> 5;
        // SAFETY: u8::MAX >> 5 caps out at 7; brought up to the next power of two, we get the following values:
        // 0, 1, 2, 4, and 8, which all match a discriminant in [`ContentLengthLength`]
        let cll =
            unsafe { std::mem::transmute::<u8, ContentLengthLength>(cll_mult.next_power_of_two()) };
        let len_usize = match cll {
            ContentLengthLength::Uint8 => len as usize,
            ContentLengthLength::Uint16 => {
                ((len as u16) << 8 | reader.read_byte()? as u16) as usize
            }
            ContentLengthLength::Uint32 => {
                let mut bytes = [len, 0, 0, 0];
                bytes[1..].copy_from_slice(&reader.read_slice(3)?);
                u32::from_be_bytes(bytes) as usize
            }
            ContentLengthLength::Uint64 => {
                let mut bytes = [len, 0, 0, 0, 0, 0, 0, 0];
                bytes[1..].copy_from_slice(&reader.read_slice(7)?);
                u64::from_be_bytes(bytes).try_into()?
            }
            _ => 0,
        };
        Ok((cll, len_usize))
    }
}

impl TlsplSize for () {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        0
    }
}

impl TlsplSerialize for () {
    #[inline]
    fn tlspl_serialize_to<W: Write>(
        &self,
        _writer: &mut W,
    ) -> crate::error::TlsplWriteResult<usize> {
        Ok(0)
    }
}

impl<'tlspl> TlsplDeserialize<'tlspl> for () {
    #[inline]
    fn tlspl_deserialize_from<R: Read<'tlspl>>(
        _reader: &mut R,
    ) -> crate::error::TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        Ok(())
    }
}

impl<T> TlsplSize for PhantomData<T> {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        0
    }
}

impl<T> TlsplSerialize for PhantomData<T> {
    #[inline]
    fn tlspl_serialize_to<W: Write>(
        &self,
        _writer: &mut W,
    ) -> crate::error::TlsplWriteResult<usize> {
        Ok(0)
    }
}

impl<'tlspl, T> TlsplDeserialize<'tlspl> for PhantomData<T> {
    #[inline]
    fn tlspl_deserialize_from<R: Read<'tlspl>>(
        _reader: &mut R,
    ) -> crate::error::TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        Ok(PhantomData)
    }
}
