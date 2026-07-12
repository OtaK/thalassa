use std::borrow::Cow;

use crate::{
    TlsplDeserialize, TlsplSerialize, TlsplSize,
    error::{TlsplReadResult, TlsplWriteResult},
    types::ContentLengthLength,
};

#[inline]
fn cl<T: TlsplSize>(slice: &[T]) -> usize {
    slice.iter().map(T::tlspl_serialized_len).sum()
}

impl<T: TlsplSize> TlsplSize for &[T] {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        let cl = cl(self);
        ContentLengthLength::from_content_len(cl) as u8 as usize + cl
    }
}

impl<'a, T: TlsplSize> TlsplSize for Cow<'a, [T]>
where
    [T]: ToOwned,
{
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        (&**self).tlspl_serialized_len()
    }
}

impl<T: TlsplSize> TlsplSize for Vec<T> {
    #[inline]
    fn tlspl_serialized_len(&self) -> usize {
        self.as_slice().tlspl_serialized_len()
    }
}

// TODO: Once specialization lands, add an impl for &[u8] to avoid the iter.fold bs. In that case we can just call slice.len()
impl<T: TlsplSerialize> TlsplSerialize for &[T] {
    #[inline]
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        let cl = cl(self);
        let cl_len = ContentLengthLength::from_content_len(cl);
        let written = self
            .iter()
            .try_fold(cl_len.write_content_len(cl, writer)?, |acc, item| {
                item.tlspl_serialize_to(writer).map(|il| acc + il)
            })?;

        debug_assert_eq!(written, self.tlspl_serialized_len(), "Write mismatch");

        Ok(written)
    }
}

impl<'a, T: TlsplDeserialize<'a> + 'a> TlsplDeserialize<'a> for Cow<'a, [T]>
where
    [T]: ToOwned<Owned = Vec<T>>,
{
    #[inline]
    fn tlspl_deserialize_from<R: crate::io::Read<'a>>(reader: &mut R) -> TlsplReadResult<Self>
    where
        Self: Sized + 'a,
    {
        let (_, length) = ContentLengthLength::read_content_len(reader)?;
        let mut checkpointed_reader = reader.checkpoint();
        let mut values = Vec::with_capacity(4);
        while checkpointed_reader.amt_read() < length {
            let item = T::tlspl_deserialize_from(&mut checkpointed_reader)?;
            values.push(item);
        }

        Ok(Cow::Owned(values))
    }
}

impl<'a, T: TlsplSerialize> TlsplSerialize for Cow<'a, [T]>
where
    [T]: ToOwned<Owned = Vec<T>>,
{
    #[inline]
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        match self {
            Cow::Borrowed(slice) => slice.tlspl_serialize_to(writer),
            Cow::Owned(vec) => vec.as_slice().tlspl_serialize_to(writer),
        }
    }
}

impl<T: TlsplSerialize> TlsplSerialize for Vec<T> {
    #[inline]
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        self.as_slice().tlspl_serialize_to(writer)
    }
}

impl<'a, T: TlsplDeserialize<'a> + 'a> TlsplDeserialize<'a> for Vec<T>
where
    [T]: ToOwned<Owned = Vec<T>>,
{
    #[inline]
    fn tlspl_deserialize_from<R: crate::io::Read<'a>>(reader: &mut R) -> TlsplReadResult<Self>
    where
        Self: Sized + 'a,
    {
        Cow::<[T]>::tlspl_deserialize_from(reader).map(Cow::into_owned)
    }
}

impl<A, B> TlsplSize for (A, B)
where
    A: TlsplSize,
    B: TlsplSize,
{
    fn tlspl_serialized_len(&self) -> usize {
        self.0.tlspl_serialized_len() + self.1.tlspl_serialized_len()
    }
}

impl<A, B> TlsplSerialize for (A, B)
where
    A: TlsplSerialize,
    B: TlsplSerialize,
{
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        Ok(self.0.tlspl_serialize_to(writer)? + self.1.tlspl_serialize_to(writer)?)
    }
}

impl<'tlspl, A, B> TlsplDeserialize<'tlspl> for (A, B)
where
    A: TlsplDeserialize<'tlspl>,
    B: TlsplDeserialize<'tlspl>,
{
    fn tlspl_deserialize_from<R: crate::io::Read<'tlspl>>(reader: &mut R) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        Ok((
            A::tlspl_deserialize_from(reader)?,
            B::tlspl_deserialize_from(reader)?,
        ))
    }
}

impl<A, B, C> TlsplSize for (A, B, C)
where
    A: TlsplSize,
    B: TlsplSize,
    C: TlsplSize,
{
    fn tlspl_serialized_len(&self) -> usize {
        self.0.tlspl_serialized_len()
            + self.1.tlspl_serialized_len()
            + self.2.tlspl_serialized_len()
    }
}

impl<A, B, C> TlsplSerialize for (A, B, C)
where
    A: TlsplSerialize,
    B: TlsplSerialize,
    C: TlsplSerialize,
{
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        Ok(self.0.tlspl_serialize_to(writer)?
            + self.1.tlspl_serialize_to(writer)?
            + self.2.tlspl_serialize_to(writer)?)
    }
}

impl<'tlspl, A, B, C> TlsplDeserialize<'tlspl> for (A, B, C)
where
    A: TlsplDeserialize<'tlspl>,
    B: TlsplDeserialize<'tlspl>,
    C: TlsplDeserialize<'tlspl>,
{
    fn tlspl_deserialize_from<R: crate::io::Read<'tlspl>>(reader: &mut R) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        Ok((
            A::tlspl_deserialize_from(reader)?,
            B::tlspl_deserialize_from(reader)?,
            C::tlspl_deserialize_from(reader)?,
        ))
    }
}

impl<A, B, C, D> TlsplSize for (A, B, C, D)
where
    A: TlsplSize,
    B: TlsplSize,
    C: TlsplSize,
    D: TlsplSize,
{
    fn tlspl_serialized_len(&self) -> usize {
        self.0.tlspl_serialized_len()
            + self.1.tlspl_serialized_len()
            + self.2.tlspl_serialized_len()
            + self.3.tlspl_serialized_len()
    }
}

impl<A, B, C, D> TlsplSerialize for (A, B, C, D)
where
    A: TlsplSerialize,
    B: TlsplSerialize,
    C: TlsplSerialize,
    D: TlsplSerialize,
{
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        Ok(self.0.tlspl_serialize_to(writer)?
            + self.1.tlspl_serialize_to(writer)?
            + self.2.tlspl_serialize_to(writer)?
            + self.3.tlspl_serialize_to(writer)?)
    }
}

impl<'tlspl, A, B, C, D> TlsplDeserialize<'tlspl> for (A, B, C, D)
where
    A: TlsplDeserialize<'tlspl>,
    B: TlsplDeserialize<'tlspl>,
    C: TlsplDeserialize<'tlspl>,
    D: TlsplDeserialize<'tlspl>,
{
    fn tlspl_deserialize_from<R: crate::io::Read<'tlspl>>(reader: &mut R) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        Ok((
            A::tlspl_deserialize_from(reader)?,
            B::tlspl_deserialize_from(reader)?,
            C::tlspl_deserialize_from(reader)?,
            D::tlspl_deserialize_from(reader)?,
        ))
    }
}

impl<A, B, C, D, E> TlsplSize for (A, B, C, D, E)
where
    A: TlsplSize,
    B: TlsplSize,
    C: TlsplSize,
    D: TlsplSize,
    E: TlsplSize,
{
    fn tlspl_serialized_len(&self) -> usize {
        self.0.tlspl_serialized_len()
            + self.1.tlspl_serialized_len()
            + self.2.tlspl_serialized_len()
            + self.3.tlspl_serialized_len()
            + self.4.tlspl_serialized_len()
    }
}

impl<A, B, C, D, E> TlsplSerialize for (A, B, C, D, E)
where
    A: TlsplSerialize,
    B: TlsplSerialize,
    C: TlsplSerialize,
    D: TlsplSerialize,
    E: TlsplSerialize,
{
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        Ok(self.0.tlspl_serialize_to(writer)?
            + self.1.tlspl_serialize_to(writer)?
            + self.2.tlspl_serialize_to(writer)?
            + self.3.tlspl_serialize_to(writer)?
            + self.4.tlspl_serialize_to(writer)?)
    }
}

impl<'tlspl, A, B, C, D, E> TlsplDeserialize<'tlspl> for (A, B, C, D, E)
where
    A: TlsplDeserialize<'tlspl>,
    B: TlsplDeserialize<'tlspl>,
    C: TlsplDeserialize<'tlspl>,
    D: TlsplDeserialize<'tlspl>,
    E: TlsplDeserialize<'tlspl>,
{
    fn tlspl_deserialize_from<R: crate::io::Read<'tlspl>>(reader: &mut R) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        Ok((
            A::tlspl_deserialize_from(reader)?,
            B::tlspl_deserialize_from(reader)?,
            C::tlspl_deserialize_from(reader)?,
            D::tlspl_deserialize_from(reader)?,
            E::tlspl_deserialize_from(reader)?,
        ))
    }
}
