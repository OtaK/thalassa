#![allow(dead_code)]

use std::borrow::Cow;
use thalassa_derive::{TlsplDeserialize, TlsplSerialize, TlsplSize};

mod custom {
    use thalassa::error::{TlsplReadResult, TlsplWriteResult};
    use thalassa::io::{DepthAwareReaderGuard, Read, Write};
    use thalassa::{TlsplDeserialize, TlsplSerialize, TlsplSize};

    #[derive(Debug, PartialEq, Clone, Copy, Default)]
    pub struct Wrapped(pub u16);

    pub fn tlspl_serialized_len(value: &Wrapped) -> usize {
        value.0.tlspl_serialized_len()
    }

    pub fn tlspl_serialize_to<W: Write>(
        value: &Wrapped,
        writer: &mut W,
    ) -> TlsplWriteResult<usize> {
        value.0.tlspl_serialize_to(writer)
    }

    pub fn tlspl_deserialize_from<'a, R: Read<'a>>(
        reader: &mut DepthAwareReaderGuard<'_, R>,
    ) -> TlsplReadResult<Wrapped> {
        Ok(Wrapped(u16::tlspl_deserialize_from(reader)?))
    }
}

#[derive(Debug, PartialEq, TlsplDeserialize, TlsplSerialize, TlsplSize)]
struct WithModule {
    #[tlspl(with = custom)]
    wrapped: custom::Wrapped,
    tail: u16,
}

#[derive(Debug, PartialEq, TlsplDeserialize, TlsplSerialize, TlsplSize)]
#[tlspl(extensible)]
#[repr(u16)]
enum ExtensibleWithModule<'a> {
    Known {
        #[tlspl(with = custom)]
        wrapped: custom::Wrapped,
        blob: Cow<'a, [u8]>,
    },
}

fn main() {}
