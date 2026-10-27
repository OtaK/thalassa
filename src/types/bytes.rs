use std::borrow::Cow;

use parsio::{CowDeque, DepthAwareReaderGuard, Read as _};

use crate::{
    TlsplDeserialize, TlsplSerialize, TlsplSize,
    error::{TlsplReadResult, TlsplWriteResult},
    types::ContentLengthLength,
};

impl<const N: usize> TlsplSize for [u8; N] {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        N
    }
}

/// This gets output as fixed-sized bytes, not VLBytes!
impl<const N: usize> TlsplSerialize for [u8; N] {
    #[inline]
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        writer.write_all(&self[..])?;
        Ok(N)
    }
}

impl<'a, const N: usize> TlsplSize for Cow<'a, [u8; N]> {
    fn tlspl_serialized_len(&self) -> usize {
        (**self).tlspl_serialized_len()
    }
}

/// Reads N bytes, not VLBytes!
impl<'tlspl, const N: usize> TlsplDeserialize<'tlspl> for Cow<'tlspl, [u8; N]> {
    #[inline]
    fn tlspl_deserialize_from<'a, R: crate::io::Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, R>,
    ) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        reader.read_array().map_err(Into::into)
    }
}

impl<'tlspl, const N: usize> TlsplDeserialize<'tlspl> for [u8; N] {
    #[inline]
    fn tlspl_deserialize_from<'a, R: crate::io::Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, R>,
    ) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        reader.read_array().map(|cow| *cow).map_err(Into::into)
    }
}

impl TlsplSize for &[u8] {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        ContentLengthLength::from_content_len(self.len()) as u8 as usize + self.len()
    }
}

impl<'a> TlsplSize for Cow<'a, [u8]> {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        (&**self).tlspl_serialized_len()
    }
}

impl<'a> TlsplSize for CowDeque<'a> {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        (&**self).tlspl_serialized_len()
    }
}

impl TlsplSize for Vec<u8> {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        self.as_slice().tlspl_serialized_len()
    }
}

impl TlsplSerialize for &[u8] {
    #[inline]
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        let cl_len = ContentLengthLength::from_content_len(self.len());
        let mut written = cl_len.write_content_len(self.len(), writer)?;
        writer.write_all(self)?;
        written += self.len();

        debug_assert_eq!(written, self.tlspl_serialized_len(), "Write mismatch");

        Ok(written)
    }
}

impl<'a> TlsplSerialize for Cow<'a, [u8]> {
    #[inline]
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        (&**self).tlspl_serialize_to(writer)
    }
}

impl<'a> TlsplSerialize for CowDeque<'a> {
    #[inline]
    fn tlspl_serialize_to<W: parsio::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        (&**self).tlspl_serialize_to(writer)
    }
}

impl TlsplSerialize for Vec<u8> {
    #[inline]
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        self.as_slice().tlspl_serialize_to(writer)
    }
}

impl<'tlspl> TlsplDeserialize<'tlspl> for Cow<'tlspl, [u8]> {
    #[inline]
    fn tlspl_deserialize_from<'a, R: crate::io::Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, R>,
    ) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        let length = ContentLengthLength::read_content_len(reader)?;
        reader.read_slice(length).map_err(Into::into)
    }
}

impl<'tlspl> TlsplDeserialize<'tlspl> for CowDeque<'tlspl> {
    #[inline]
    fn tlspl_deserialize_from<'a, R: crate::io::Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, R>,
    ) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        let length = ContentLengthLength::read_content_len(reader)?;
        reader
            .read_slice(length)
            .map_err(Into::into)
            .map(Into::into)
    }
}

impl<'tlspl> TlsplDeserialize<'tlspl> for Vec<u8> {
    #[inline]
    fn tlspl_deserialize_from<'a, R: crate::io::Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, R>,
    ) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        Cow::<[u8]>::tlspl_deserialize_from(reader).map(Cow::into_owned)
    }
}

#[cfg(test)]
mod tests {
    use crate::io::DepthAwareReader;
    use crate::{TlsplDeserialize, TlsplSerialize, TlsplSize};

    fn round_trip(len: usize) {
        let payload: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
        let bytes = payload.tlspl_serialize().expect("serialize");
        assert_eq!(bytes.len(), payload.tlspl_serialized_len());

        let mut reader = DepthAwareReader::from_reader(bytes.as_slice());
        let mut guard = reader.enter().expect("enter");
        let decoded = Vec::<u8>::tlspl_deserialize_from(&mut guard).expect("deserialize");
        assert_eq!(decoded, payload, "round-trip differs at {len} bytes");
    }

    #[test]
    fn vlbytes_round_trips_across_length_classes() {
        for len in [0usize, 1, 0x3f, 0x40, 0x41, 0x3fff, 0x4000, 0x4001, 1 << 20] {
            round_trip(len);
        }
    }
}
