use std::borrow::Cow;

use parsio::{DepthAwareReaderGuard, Read as _};

use crate::{
    TlsplDeserialize, TlsplSerialize, TlsplSize,
    error::{TlsplReadError, TlsplReadResult, TlsplWriteResult},
    types::ContentLengthLength,
};

#[inline]
fn cl<T: TlsplSize>(slice: &[T]) -> usize {
    slice.iter().map(T::tlspl_serialized_len).sum()
}

fn content_length_sanity_check(
    decl_len: usize,
    before_after: Option<(usize, usize)>,
    amt_read: usize,
) -> TlsplReadResult<()> {
    // Sanity checks on lengths
    if let Some((before, after)) = before_after {
        let got = before.saturating_sub(after);
        if got != decl_len {
            return Err(TlsplReadError::ContentLengthMismatch {
                expected: decl_len,
                got,
            });
        }
    } else if amt_read != decl_len {
        return Err(TlsplReadError::ContentLengthMismatch {
            expected: decl_len,
            got: amt_read, // that's the declared sum (tlspl_serialized_len) so don't trust it too much
        });
    }

    Ok(())
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

impl<'tlspl, T: TlsplDeserialize<'tlspl> + 'tlspl> TlsplDeserialize<'tlspl> for Cow<'tlspl, [T]>
where
    [T]: ToOwned<Owned = Vec<T>>,
{
    #[inline]
    fn tlspl_deserialize_from<'a, R: crate::io::Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, 'tlspl, R>,
    ) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        Vec::<T>::tlspl_deserialize_from(reader).map(Cow::Owned)
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
            Cow::Owned(vec) => vec.tlspl_serialize_to(writer),
        }
    }
}

impl<T: TlsplSerialize> TlsplSerialize for Vec<T> {
    #[inline]
    fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        self.as_slice().tlspl_serialize_to(writer)
    }
}

impl<'tlspl, T: TlsplDeserialize<'tlspl> + 'tlspl> TlsplDeserialize<'tlspl> for Vec<T> {
    #[inline]
    fn tlspl_deserialize_from<'a, R: crate::io::Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, 'tlspl, R>,
    ) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        let length = ContentLengthLength::read_content_len(reader)?;
        let mut values = Vec::with_capacity(4);
        let before = reader.remaining();
        let mut reader = reader.enter()?;
        let mut amt_read = 0usize;
        while amt_read < length {
            let prev = amt_read;
            let item = T::tlspl_deserialize_from(&mut reader)?;
            amt_read += item.tlspl_serialized_len();
            if amt_read == prev {
                return Err(TlsplReadError::ZeroSizedCollectionElement);
            }

            values.push(item);
        }

        content_length_sanity_check(length, before.zip(reader.remaining()), amt_read)?;

        Ok(values)
    }
}

impl<V: TlsplSize> TlsplSize for std::collections::HashSet<V> {
    fn tlspl_serialized_len(&self) -> usize {
        let cl = self.iter().map(V::tlspl_serialized_len).sum();
        ContentLengthLength::from_content_len(cl) as u8 as usize + cl
    }
}

impl<V: TlsplSerialize> TlsplSerialize for std::collections::HashSet<V> {
    fn tlspl_serialize_to<W: parsio::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        let cl = self.iter().map(V::tlspl_serialized_len).sum();
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

impl<'tlspl, V: TlsplDeserialize<'tlspl> + std::hash::Hash + Eq + 'tlspl> TlsplDeserialize<'tlspl>
    for std::collections::HashSet<V>
{
    fn tlspl_deserialize_from<'a, R: parsio::Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, 'tlspl, R>,
    ) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        let length = ContentLengthLength::read_content_len(reader)?;
        let mut amt_read = 0usize;
        let mut values = std::collections::HashSet::new();
        let before = reader.remaining();
        let mut reader = reader.enter()?;
        while amt_read < length {
            let prev = amt_read;
            let item = V::tlspl_deserialize_from(&mut reader)?;
            amt_read += item.tlspl_serialized_len();
            if amt_read == prev {
                return Err(TlsplReadError::ZeroSizedCollectionElement);
            }

            let is_new = values.insert(item);
            if !is_new {
                return Err(TlsplReadError::DuplicateSetValue);
            }
        }

        content_length_sanity_check(length, before.zip(reader.remaining()), amt_read)?;

        Ok(values)
    }
}

impl<V: TlsplSize> TlsplSize for std::collections::BTreeSet<V> {
    fn tlspl_serialized_len(&self) -> usize {
        let cl = self.iter().map(V::tlspl_serialized_len).sum();
        ContentLengthLength::from_content_len(cl) as u8 as usize + cl
    }
}

impl<V: TlsplSerialize> TlsplSerialize for std::collections::BTreeSet<V> {
    fn tlspl_serialize_to<W: parsio::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        let cl = self.iter().map(V::tlspl_serialized_len).sum();
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

impl<'tlspl, V: TlsplDeserialize<'tlspl> + Ord + 'tlspl> TlsplDeserialize<'tlspl>
    for std::collections::BTreeSet<V>
{
    fn tlspl_deserialize_from<'a, R: parsio::Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, 'tlspl, R>,
    ) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        let length = ContentLengthLength::read_content_len(reader)?;
        let mut amt_read = 0usize;
        let mut values = std::collections::BTreeSet::new();
        let before = reader.remaining();
        let mut reader = reader.enter()?;
        while amt_read < length {
            let prev = amt_read;
            let item = V::tlspl_deserialize_from(&mut reader)?;
            amt_read += item.tlspl_serialized_len();
            if amt_read == prev {
                return Err(TlsplReadError::ZeroSizedCollectionElement);
            }

            let is_new = values.insert(item);
            if !is_new {
                return Err(TlsplReadError::DuplicateSetValue);
            }
        }

        content_length_sanity_check(length, before.zip(reader.remaining()), amt_read)?;

        Ok(values)
    }
}

impl<K: TlsplSize, V: TlsplSize> TlsplSize for std::collections::HashMap<K, V> {
    fn tlspl_serialized_len(&self) -> usize {
        let cl = self
            .iter()
            .map(|(k, v)| k.tlspl_serialized_len() + v.tlspl_serialized_len())
            .sum();
        let cll = ContentLengthLength::from_content_len(cl);
        cll as u8 as usize + cl
    }
}

impl<K: TlsplSerialize, V: TlsplSerialize> TlsplSerialize for std::collections::HashMap<K, V> {
    fn tlspl_serialize_to<W: parsio::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        let cl = self
            .iter()
            .map(|(k, v)| k.tlspl_serialized_len() + v.tlspl_serialized_len())
            .sum();
        let cll = ContentLengthLength::from_content_len(cl);
        let mut written = cll.write_content_len(cl, writer)?;
        for pair in self.iter() {
            written += pair.tlspl_serialize_to(writer)?
        }

        Ok(written)
    }
}

impl<
    'tlspl,
    K: TlsplDeserialize<'tlspl> + 'tlspl + std::hash::Hash + Eq,
    V: TlsplDeserialize<'tlspl> + 'tlspl,
> TlsplDeserialize<'tlspl> for std::collections::HashMap<K, V>
{
    #[inline]
    fn tlspl_deserialize_from<'a, R: crate::io::Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, 'tlspl, R>,
    ) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        let length = ContentLengthLength::read_content_len(reader)?;
        let mut amt_read = 0usize;
        let mut pairs = Self::default();
        let before = reader.remaining();
        let mut reader = reader.enter()?;

        while amt_read < length {
            let prev = amt_read;
            let k = K::tlspl_deserialize_from(&mut reader)?;
            let v = V::tlspl_deserialize_from(&mut reader)?;

            amt_read += k.tlspl_serialized_len() + v.tlspl_serialized_len();

            if amt_read == prev {
                return Err(TlsplReadError::ZeroSizedCollectionElement);
            }

            if pairs.insert(k, v).is_some() {
                return Err(TlsplReadError::DuplicateMapKey);
            }
        }

        content_length_sanity_check(length, before.zip(reader.remaining()), amt_read)?;

        Ok(pairs)
    }
}

impl<K: TlsplSize, V: TlsplSize> TlsplSize for std::collections::BTreeMap<K, V> {
    fn tlspl_serialized_len(&self) -> usize {
        let cl = self
            .iter()
            .map(|(k, v)| k.tlspl_serialized_len() + v.tlspl_serialized_len())
            .sum();
        let cll = ContentLengthLength::from_content_len(cl);
        cll as u8 as usize + cl
    }
}

impl<K: TlsplSerialize, V: TlsplSerialize> TlsplSerialize for std::collections::BTreeMap<K, V> {
    fn tlspl_serialize_to<W: parsio::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
        let cl = self
            .iter()
            .map(|(k, v)| k.tlspl_serialized_len() + v.tlspl_serialized_len())
            .sum();
        let cll = ContentLengthLength::from_content_len(cl);
        let mut written = cll.write_content_len(cl, writer)?;
        for pair in self.iter() {
            written += pair.tlspl_serialize_to(writer)?
        }

        Ok(written)
    }
}

impl<'tlspl, K: TlsplDeserialize<'tlspl> + 'tlspl + Ord, V: TlsplDeserialize<'tlspl> + 'tlspl>
    TlsplDeserialize<'tlspl> for std::collections::BTreeMap<K, V>
{
    #[inline]
    fn tlspl_deserialize_from<'a, R: crate::io::Read<'tlspl>>(
        reader: &mut DepthAwareReaderGuard<'_, 'tlspl, R>,
    ) -> TlsplReadResult<Self>
    where
        Self: Sized + 'tlspl,
    {
        let length = ContentLengthLength::read_content_len(reader)?;
        let mut amt_read = 0usize;
        let mut pairs = Self::default();
        let before = reader.remaining();
        let mut reader = reader.enter()?;
        while amt_read < length {
            let prev = amt_read;
            let k = K::tlspl_deserialize_from(&mut reader)?;
            let v = V::tlspl_deserialize_from(&mut reader)?;

            amt_read += k.tlspl_serialized_len() + v.tlspl_serialized_len();

            if amt_read == prev {
                return Err(TlsplReadError::ZeroSizedCollectionElement);
            }

            if pairs.insert(k, v).is_some() {
                return Err(TlsplReadError::DuplicateMapKey);
            }
        }

        content_length_sanity_check(length, before.zip(reader.remaining()), amt_read)?;

        Ok(pairs)
    }
}

macro_rules! tuple_impls {
    ($($len:tt => ($($n:tt $g:ident)+))+) => {
        $(
            impl<$($g),+> TlsplSize for ($($g,)+)
            where
                $($g: TlsplSize,)+
            {
                #[inline]
                fn tlspl_serialized_len(&self) -> usize {
                    $(self.$n.tlspl_serialized_len() + )+ 0
                }
            }

            impl<$($g),+> TlsplSerialize for ($($g,)+)
            where
                $($g: TlsplSerialize,)+
            {
                #[inline]
                fn tlspl_serialize_to<W: crate::io::Write>(&self, writer: &mut W) -> TlsplWriteResult<usize> {
                    Ok($(self.$n.tlspl_serialize_to(writer)? + )+ 0)
                }
            }

            impl<'tlspl, $($g),+> TlsplDeserialize<'tlspl> for ($($g),+)
            where
                $($g: TlsplDeserialize<'tlspl> + 'tlspl),+
            {
                #[inline]
                fn tlspl_deserialize_from<R: crate::io::Read<'tlspl>>(reader: &mut DepthAwareReaderGuard<'_, 'tlspl, R>) -> TlsplReadResult<Self> {
                    Ok((
                        $($g::tlspl_deserialize_from(reader)?,)+
                    ))
                }
            }
        )+
    };
}

tuple_impls! {
    2 => (0 A 1 B)
    3 => (0 A 1 B 2 C)
    4 => (0 A 1 B 2 C 3 D)
    5 => (0 A 1 B 2 C 3 D 4 E)
}
