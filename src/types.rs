use std::marker::PhantomData;

use parsio::DepthAwareReaderGuard;

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

// This is using the MLS limits on all platforms: 2^30-1
// The classic TLSPL limits are stupid anyway, 2^62-1 is absurd and
// would lead to OoM on pretty much any computer on earth: it's 4.6 exabytes.
const MAX_LEN: usize = (1 << 30) - 1;

/// Returns the TLS VarInt overhead for the provided content length
///
/// It's usually used to AoT compute the length of the sum of serialized things
/// that are collated as a variable-length bytes thingy
#[inline]
pub const fn content_len_as_vlbytes_overhead(cl: usize) -> usize {
    ContentLengthLength::from_content_len(cl) as u8 as usize
}

/// Writes a "ContentLengthLength" VLBytes prefix on the writer.
///
/// Currently used by the derive macro to avoid buffering when serializing `#[tlspl(extensible)]` enums
#[doc(hidden)]
#[inline]
pub fn write_vlbytes_content_len<W: Write>(
    cl: usize,
    writer: &mut W,
) -> crate::error::TlsplWriteResult<usize> {
    ContentLengthLength::from_content_len(cl).write_content_len(cl, writer)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ContentLengthLength {
    #[default]
    Empty = 0,
    Uint8 = 1,
    Uint16 = 2,
    Uint32 = 4,
    Uint64 = 8,
}

const VARINT_U16_MARKER: u8 = 0x40;
const VARINT_U32_MARKER: u8 = 0x80;
const VARINT_U64_MARKER: u8 = 0xC0;

const CL_U8_MAX: usize = 0x40;
const CL_U16_MAX: usize = 0x4000;
const CL_U32_MAX: usize = 0x4000_0000;

impl ContentLengthLength {
    #[inline]
    pub const fn from_content_len(cl: usize) -> Self {
        if cl > MAX_LEN {
            return Self::Empty;
        }

        if cl < CL_U8_MAX {
            Self::Uint8
        } else if cl < CL_U16_MAX {
            Self::Uint16
        } else if cl < CL_U32_MAX {
            Self::Uint32
        } else {
            Self::Uint64
        }
    }

    #[inline]
    pub fn overhead(&self) -> usize {
        *self as u8 as usize
    }

    #[inline]
    pub fn write_content_len<W: Write>(
        &self,
        cl: usize,
        writer: &mut W,
    ) -> crate::error::TlsplWriteResult<usize> {
        if cl > MAX_LEN {
            return Err(crate::error::TlsplWriteError::LengthOverflow);
        }

        debug_assert_eq!(*self, Self::from_content_len(cl), "Internal API misuse");

        match self {
            ContentLengthLength::Empty => return Ok(0),
            ContentLengthLength::Uint8 => {
                writer.write_all(&[cl as u8])?;
            }
            ContentLengthLength::Uint16 => {
                writer
                    .write_all(&(((VARINT_U16_MARKER as u16) << 8) | (cl as u16)).to_be_bytes())?;
            }
            ContentLengthLength::Uint32 => {
                writer
                    .write_all(&(((VARINT_U32_MARKER as u32) << 24) | (cl as u32)).to_be_bytes())?;
            }
            ContentLengthLength::Uint64 => {
                writer
                    .write_all(&(((VARINT_U64_MARKER as u64) << 56) | (cl as u64)).to_be_bytes())?;
            }
        }

        Ok(*self as u8 as usize)
    }

    #[inline]
    pub fn read_content_len<'a, R: Read<'a>>(
        reader: &mut R,
    ) -> crate::error::TlsplReadResult<usize> {
        let lb = reader.read_byte()?;
        let len_fb = lb & 0x3F; // Mask the first byte as it'll be used like that in all branches
        let cl = if lb < VARINT_U16_MARKER {
            len_fb as usize
        } else if lb < VARINT_U32_MARKER {
            ((len_fb as u16) << 8 | reader.read_byte()? as u16) as usize
        } else if lb < VARINT_U64_MARKER {
            let mut bytes = [len_fb, 0, 0, 0];
            bytes[1..].copy_from_slice(&*reader.read_array::<3>()?);
            u32::from_be_bytes(bytes) as usize
        } else {
            let mut bytes = [len_fb, 0, 0, 0, 0, 0, 0, 0];
            bytes[1..].copy_from_slice(&*reader.read_array::<7>()?);
            u64::from_be_bytes(bytes).try_into()?
        };

        if cl > MAX_LEN {
            return Err(crate::error::TlsplReadError::LengthOverflow);
        }

        Ok(cl)
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
        _reader: &mut DepthAwareReaderGuard<'_, R>,
    ) -> crate::error::TlsplReadResult<Self> {
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
        _reader: &mut DepthAwareReaderGuard<'_, R>,
    ) -> crate::error::TlsplReadResult<Self> {
        Ok(PhantomData)
    }
}
