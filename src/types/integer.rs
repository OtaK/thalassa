use crate::{
    TlsplDeserialize, TlsplSerialize, TlsplSize,
    error::{TlsplReadResult, TlsplWriteResult},
};

macro_rules! impl_int {
    ($repr:ty) => {
        impl TlsplSize for $repr {
            #[inline]
            fn tlspl_serialized_len(&self) -> usize {
                (<$repr>::BITS / 8) as usize
            }
        }

        impl TlsplSerialize for $repr {
            #[inline]
            fn tlspl_serialize_to<W: crate::io::Write>(
                &self,
                writer: &mut W,
            ) -> TlsplWriteResult<usize> {
                writer.write(&self.to_be_bytes()).map_err(Into::into)
            }
        }

        impl<'a> TlsplDeserialize<'a> for $repr {
            #[inline]
            fn tlspl_deserialize_from<R: crate::io::Read<'a>>(
                reader: &mut R,
            ) -> TlsplReadResult<Self>
            where
                Self: Sized + 'a,
            {
                reader
                    .read_array()
                    .map(|b| <$repr>::from_be_bytes(*b))
                    .map_err(Into::into)
            }
        }
    };
}

// This conflicts with the &[u8] specialization since Serialize for &[T] covers &[u8] since u8 impls Serialize
// impl_int!(u8);
impl_int!(u16);
impl_int!(u32);
impl_int!(u64);
impl_int!(u128);
impl_int!(usize);
impl_int!(i8);
impl_int!(i16);
impl_int!(i32);
impl_int!(i64);
impl_int!(i128);
impl_int!(isize);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
#[allow(non_camel_case_types)]
// 24-bit integer, useful for TLS itself
//
// Is internally backed by a u32 but with some guardrails to prevent misuse (eg: upper byte is masked on conversion from u32)
pub struct u24(u32);

impl TlsplSize for u24 {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        3
    }
}

impl TlsplSerialize for u24 {
    #[inline]
    fn tlspl_serialize_to<W: parsio::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        // First byte is always 0x00 for a u24
        writer.write(&self.0.to_be_bytes()[1..]).map_err(Into::into)
    }
}

impl<'a> TlsplDeserialize<'a> for u24 {
    fn tlspl_deserialize_from<R: parsio::Read<'a>>(reader: &mut R) -> TlsplReadResult<Self>
    where
        Self: Sized + 'a,
    {
        let array: [u8; 3] = *reader.read_array()?;
        Ok(Self(u32::from_be_bytes([
            0x00, array[0], array[1], array[2],
        ])))
    }
}

impl From<u32> for u24 {
    fn from(value: u32) -> Self {
        // Mask the upper byte
        Self(value & 0x00_FF_FF_FF)
    }
}

impl std::ops::Deref for u24 {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TlsplSize for bool {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        1
    }
}

impl TlsplSerialize for bool {
    #[inline]
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        writer.write(&[*self as u8]).map_err(Into::into)
    }
}

impl<'a> TlsplDeserialize<'a> for bool {
    #[inline]
    fn tlspl_deserialize_from<R: crate::io::Read<'a>>(reader: &mut R) -> TlsplReadResult<Self>
    where
        Self: Sized + 'a,
    {
        reader.read_byte().map(|b| b > 0).map_err(Into::into)
    }
}
