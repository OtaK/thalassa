use crate::{
    TlsplDeserialize, TlsplSerialize, TlsplSize,
    error::{TlsplReadResult, TlsplWriteResult},
};

impl<T: TlsplSize> TlsplSize for Option<T> {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        // 1 is the extra byte for the [0|1] discriminant of optional<V>
        1 + self
            .as_ref()
            .map(T::tlspl_serialized_len)
            .unwrap_or_default()
    }
}

impl<T: TlsplSerialize> TlsplSerialize for Option<T> {
    #[inline]
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        // Write 0 or 1 depending on the presence of the value
        let mut written = writer.write(&[self.is_some() as u8])?;
        debug_assert_eq!(
            written, 1,
            "Could not even write the optional<V> byte, something is awry"
        );

        if let Some(value) = self {
            written += value.tlspl_serialize_to(writer)?;
        }
        Ok(written)
    }
}

impl<'tlspl, T: TlsplDeserialize<'tlspl> + 'tlspl> TlsplDeserialize<'tlspl> for Option<T> {
    #[inline]
    fn tlspl_deserialize_from<R: crate::io::Read<'tlspl>>(reader: &mut R) -> TlsplReadResult<Self> {
        (reader.read_byte()? >= 0x01)
            .then(|| T::tlspl_deserialize_from(reader))
            .transpose()
    }
}
