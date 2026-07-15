use std::borrow::Cow;

use crate::{
    TlsplDeserialize, TlsplSerialize, TlsplSize,
    error::{TlsplReadResult, TlsplWriteResult},
};

impl TlsplSize for &str {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        self.as_bytes().tlspl_serialized_len()
    }
}

impl<'a> TlsplSize for Cow<'a, str> {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        (&**self).tlspl_serialized_len()
    }
}

impl TlsplSize for String {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        self.as_str().tlspl_serialized_len()
    }
}

impl TlsplSerialize for &str {
    #[inline]
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        self.as_bytes().tlspl_serialize_to(writer)
    }
}

impl<'a> TlsplSerialize for Cow<'a, str> {
    #[inline]
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        (&**self).tlspl_serialize_to(writer)
    }
}

impl TlsplSerialize for String {
    #[inline]
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        self.as_str().tlspl_serialize_to(writer)
    }
}

impl<'tlspl> TlsplDeserialize<'tlspl> for Cow<'tlspl, str> {
    #[inline]
    fn tlspl_deserialize_from<R: crate::io::Read<'tlspl>>(reader: &mut R) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        let bytes = Cow::<'tlspl, [u8]>::tlspl_deserialize_from(reader)?;
        Ok(match bytes {
            Cow::Borrowed(str) => Cow::Borrowed(simdutf8::basic::from_utf8(str)?),
            Cow::Owned(string) => {
                simdutf8::basic::from_utf8(string.as_slice())?;
                // SAFETY: If the above check validates, then it is safe to turn this into an owned `String`
                // The reason it's a 2-step process is that simdutf8 has no `String`-related API.
                Cow::Owned(unsafe { String::from_utf8_unchecked(string) })
            }
        })
    }
}

impl<'tlspl> TlsplDeserialize<'tlspl> for String {
    #[inline]
    fn tlspl_deserialize_from<R: crate::io::Read<'tlspl>>(reader: &mut R) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        Cow::<str>::tlspl_deserialize_from(reader).map(Cow::into_owned)
    }
}
