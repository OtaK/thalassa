//! [`asynchronous_codec`] glue: turns any `AsyncRead`/`AsyncWrite` into a `Stream`/`Sink` of TLSPL
//! values through [`asynchronous_codec::Framed`].
//!
//! ```no_run
//! # async fn run<T: futures::io::AsyncRead + futures::io::AsyncWrite + Unpin>(io: T) {
//! use asynchronous_codec::Framed;
//! use futures::{SinkExt as _, StreamExt as _};
//! use thalassa::codec::TlsplCodec;
//!
//! let mut framed = Framed::new(io, TlsplCodec::<Vec<u8>>::new(64 * 1024));
//! framed.send(vec![1, 2, 3]).await.unwrap();
//! let echoed: Option<Vec<u8>> = framed.next().await.transpose().unwrap();
//! # }
//! ```
//!
//! The encoder takes `T` by value and has no lifetime bound, so a write-only half can encode
//! borrowed values without copying them, through the `&T` blanket impl:
//!
//! ```no_run
//! # async fn run<W: futures::io::AsyncWrite + Unpin>(io: W, msg: &[u8]) {
//! use asynchronous_codec::FramedWrite;
//! use futures::SinkExt as _;
//! use thalassa::codec::TlsplCodec;
//!
//! let mut sink = FramedWrite::new(io, TlsplCodec::<&&[u8]>::new(64 * 1024));
//! sink.send(&msg).await.unwrap();
//! # }
//! ```

use std::marker::PhantomData;

use asynchronous_codec::{BytesMut, Decoder, Encoder};
use parsio::{DepthAwareReader, Detached, WriteResult};

use crate::{
    TlsplDeserialize, TlsplSerialize,
    error::{TlsplReadError, TlsplWriteError},
};

/// Decodes back-to-back TLSPL values of type `T` and encodes `T` (use `TlsplCodec<&U>` to encode
/// by reference).
///
/// Decoded values own their data (`T: TlsplDeserialize<'static>`): each parse copies out of the
/// read buffer.
///
/// Zero-sized item types (`()`, all-`skip` structs) are rejected: a zero-byte frame can't be told
/// apart from an empty stream, so it would never make progress.
///
/// Errors are terminal: after the first one, the decoder discards everything it is fed, so the
/// stream yields that error once, then ends at EOF.
#[derive(Debug)]
pub struct TlsplCodec<T> {
    max_frame_len: usize,
    /// Skip reparsing until this many bytes are buffered.
    needed: usize,
    poisoned: bool,
    _t: PhantomData<fn() -> T>,
}

impl<T> TlsplCodec<T> {
    /// `max_frame_len` caps the bytes buffered for a single value. Without it, a hostile length
    /// prefix would make the decoder buffer forever.
    pub fn new(max_frame_len: usize) -> Self {
        Self {
            max_frame_len,
            needed: 0,
            poisoned: false,
            _t: PhantomData,
        }
    }

    fn poison(&mut self, src: &mut BytesMut, err: TlsplReadError) -> TlsplReadError {
        self.poisoned = true;
        src.clear();
        err
    }
}

impl<T: TlsplDeserialize<'static> + 'static> Decoder for TlsplCodec<T> {
    type Item = T;
    type Error = TlsplReadError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<T>, TlsplReadError> {
        if self.poisoned {
            src.clear();
            return Ok(None);
        }
        if src.len() < self.needed {
            return Ok(None);
        }

        let mut rest = &src[..];
        let parsed = DepthAwareReader::from_reader(Detached::new(&mut rest))
            .enter()
            .map_err(TlsplReadError::from)
            .and_then(|mut guard| T::tlspl_deserialize_from(&mut guard));
        let consumed = src.len() - rest.len();

        match parsed {
            Ok(_) if consumed > self.max_frame_len => Err(self.poison(
                src,
                TlsplReadError::FrameTooLarge {
                    max: self.max_frame_len,
                },
            )),
            // Zero bytes consumed: yielding it forever would make no progress.
            Ok(_) if consumed == 0 => Err(self.poison(src, TlsplReadError::ZeroSizedFrame)),
            Ok(value) => {
                let _ = src.split_to(consumed);
                self.needed = 0;
                Ok(Some(value))
            }
            Err(e) if e.is_truncation() => {
                let needed = match e {
                    TlsplReadError::Incomplete { missing } => src.len().saturating_add(missing),
                    _ => src.len() + 1,
                };
                if needed > self.max_frame_len {
                    return Err(self.poison(
                        src,
                        TlsplReadError::FrameTooLarge {
                            max: self.max_frame_len,
                        },
                    ));
                }
                src.reserve(needed - src.len());
                self.needed = needed;
                Ok(None)
            }
            Err(e) => Err(self.poison(src, e)),
        }
    }

    fn decode_eof(&mut self, src: &mut BytesMut) -> Result<Option<T>, TlsplReadError> {
        match self.decode(src)? {
            None if !src.is_empty() => Err(self.poison(
                src,
                std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into(),
            )),
            frame => Ok(frame),
        }
    }
}

impl<T: TlsplSerialize> Encoder for TlsplCodec<T> {
    type Item<'a> = T;
    type Error = TlsplWriteError;

    fn encode(&mut self, item: T, dst: &mut BytesMut) -> Result<(), TlsplWriteError> {
        let start = dst.len();
        dst.reserve(item.tlspl_serialized_len());
        let encoded = item.tlspl_serialize_to(&mut BytesMutWriter(dst)).map(drop);
        if encoded.is_err() {
            // Roll back partial writes so a failed item can't corrupt the next frame.
            dst.truncate(start);
        }
        encoded
    }
}

struct BytesMutWriter<'a>(&'a mut BytesMut);

impl parsio::Write for BytesMutWriter<'_> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> WriteResult<usize> {
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> WriteResult<()> {
        self.0.extend_from_slice(buf);
        Ok(())
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[&[u8]]) -> WriteResult<usize> {
        let len = bufs.iter().map(|b| b.len()).sum();
        self.0.reserve(len);
        for buf in bufs {
            self.0.extend_from_slice(buf);
        }
        Ok(len)
    }

    #[inline]
    fn flush(&mut self) -> WriteResult<()> {
        Ok(())
    }
}
