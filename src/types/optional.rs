use crate::{
    TlsplDeserialize, TlsplSerialize, TlsplSize,
    error::{TlsplReadResult, TlsplWriteResult},
};

use parsio::{DepthAwareReaderGuard, Read as _};

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
        writer.write_all(&[self.is_some() as u8])?;
        let mut written = 1;
        if let Some(value) = self {
            written += value.tlspl_serialize_to(writer)?;
        }

        Ok(written)
    }
}

impl<'tlspl, T: TlsplDeserialize<'tlspl> + 'tlspl> TlsplDeserialize<'tlspl> for Option<T> {
    #[inline]
    fn tlspl_deserialize_from<'a, R: parsio::Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, R>,
    ) -> TlsplReadResult<Self> {
        Ok(match reader.read_byte()? {
            0x00 => None,
            0x01 => Some(T::tlspl_deserialize_from(&mut reader.enter()?)?),
            _ => return Err(crate::error::TlsplReadError::IllegalOptionPrefix),
        })
    }
}
