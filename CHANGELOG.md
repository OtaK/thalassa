# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## 0.1.0 - unreleased

The hardening release: a correctness pass over the codec, one redesign on the derive, depth tracking.

Nothing here changes the bytes emitted for valid data, with one exception: enums whose non-literal native discriminant diverged from Rust's numbering now encode the number Rust assigns (see *Changed*). Every other behaviour break is a *new rejection* of input that used to be accepted, so non-canonical stored or queued messages that decode today will start erroring.

### Added

- Billion Laughs protection on the read path: `TlsplDeserialize::tlspl_deserialize_from` takes `&mut DepthAwareReaderGuard<'_, 'tlspl, R>`
  - Wrap the reader with `thalassa::io::DepthAwareReader::from_reader(..)`, then `.enter()`. Default limit is set to 256, override with `from_reader_with_limit`.
  - Serialization is deliberately *not* depth-tracked, so footguns beware (I'm not your dad).
- `TlsplDeserialize::tlspl_deserialize_from_complete`: errors out unless the buffer has been entirely consumed.
- `TlsplReadError` variants: `LengthOverflow`, `DuplicateMapKey`, `IllegalBool`, `IllegalOptionPrefix`, `MissingEnumDiscriminant { enum_type }`, `TrailingBytes { extra }`, `ContentLengthMismatch { expected, got }`, `ZeroSizedCollectionElement`, `ZeroSizedFrame`, `Incomplete { missing }`, `FrameTooLarge { max }`, `NestedTruncation`.
- `futures-codec` feature: `thalassa::codec::TlsplCodec<T>` implements `asynchronous_codec::{Decoder, Encoder}`, so `Framed::new(io, TlsplCodec::new(max_frame_len))` is a `Stream<Item = Result<T, _>>` + `Sink<T>` over any futures-io `AsyncRead`/`AsyncWrite`.
  - Decoded values are owned (`T: TlsplDeserialize<'static>`), copied out of the read buffer through `parsio::Detached`.
  - `max_frame_len` is required: a length prefix announcing more than that fails right away with `FrameTooLarge` instead of buffering.
  - Truncated input waits for more bytes; any other error is terminal. After one, the codec drops everything it is fed and the stream ends at EOF.
- `TlsplReadError::is_truncation()`; `From<std::io::Error>` for `TlsplReadError` and `TlsplWriteError` (only the `ErrorKind` is kept).
- `#[diagnostic::on_unimplemented]` on `TlsplSize`/`TlsplDeserialize`/`TlsplSerialize` and `#[diagnostic::do_not_recommend]` on the derivable impls - a non-TLSPL field now says which type and why, instead of a boring `E0277`.
- Derive: expression discriminants (`SOME_CONST`, `1 + 1`, `u8::MAX`) are emitted verbatim and cross-checked against the native discriminant at const evaluation.
- Tests: Added more fixtures, and a few tests. LLM-assisted, as per the README.

### Fixed

- `bool` and `Option<T>` accepted any non-zero byte as `1`.
- Maps silently overwrote duplicate keys.
- Partial `Write::write` on every serialization path — now `write_all`, so a short sink returns `WriteZero` instead of `Ok` with a truncated payload.
- Containers accounted for the *declared* length instead of the consumed one: a boundary overrun and a zero-sized-element hang are both rejected now.
- `MAX_LEN` adopts MLS limits: went from `(1 << 62) - 2` (there was an off by one as well hihi) => `(1 << 30) - 1`.
- `#[tlspl(extensible)]`: a known variant with leftover bytes in its length-prefixed buffer returns `TrailingBytes` instead of dropping them on re-encode. The catch-all still keeps the whole buffer, and an explicit tail field round-trips.
- `#[tlspl(extensible)]` combined with a `u8` field didn't compile.
- An untagged enum read without a `select` panicked; it now returns `MissingEnumDiscriminant { enum_type }`.
- Path discriminants went through an `unsafe` pointer cast with no width or range check. See below.

### Changed

- **Breaking:** `TlsplDeserialize::tlspl_deserialize_from` now takes `&mut DepthAwareReaderGuard<'_, 'tlspl, R>`.
- **Breaking:** `#[tlspl(discriminant = ..)]` is distinguished by quoting now.
  - An *unquoted* expression is a **value** (`SOME_CONST`, `1 + 1`, `u8::MAX`) written into a `#[repr]`-typed const, so rustc type-checks it — a value that doesn't fit is an `E0308`, not a truncation.
  - A *quoted* path (`"OtherEnum::Variant"`) is a **variant read** out of another enum, the only spelling that can name a variant of an enum that has fields, since such an enum can't be cast to an integer. The read takes the *discriminant's* width and range-checks it against the target's `#[repr]`, so a wider source is refused instead of silently truncated.
  - **Migrating is mechanical**: `discriminant = "SOME_CONST"` => `discriminant = SOME_CONST`.
- **Breaking:** integer impls are `u16`/`u32`/`u64`/`u128` and `i8`..`i128`.
  - `i8`..`i128` and `u128` are extensions to TLSPL: fixed width, big-endian, two's complement for the signed ones.
  - `usize`/`isize` stay out: pointer width is not wire-stable.
  - `u8` still has none because it cannot be done (clashes with `&[u8]` impls) until specialization stabilizes in rustc.
- **Breaking:** the new error variants break exhaustive `match` on `TlsplReadError`; both `TlsplReadError` and `TlsplWriteError` are now `#[non_exhaustive]`, so future variants won't.
- **Breaking:** a vector/bytes field whose length prefix announces more bytes than an in-memory slice holds fails early with `Incomplete { missing }` instead of `Parsio(IoError(UnexpectedEof))`. Readers of unknown length (`StdReader`) are unchanged.
- `#[tlspl(extensible)]`: a variant running short inside its (complete) length-prefixed buffer returns `NestedTruncation` instead of `UnexpectedEof`/`Incomplete`, since more input can't fix it.
- Bumped `parsio` to `0.1.0-rc.7`.
- **Breaking:** input that previously decoded weirdly now errors, or encodes to different bytes:
  - non-literal native discriminants are now emitted on the wire correctly
    - For example, previously, an enum like below would emit wire discriminants of `0,1,2` while the correct, intuitive values would be `0,5,6`, since `SOME_CONST` bumps the
      implicit discriminant to `5`, causing the next discriminant (`C`) to be `6`.
      ```rust,ignore
        const SOME_CONST: u8 = 5;
        #[repr(u8)]
        enum E { A, B = SOME_CONST, C }
      ```
    - Another consequence of this, is that since the discriminants made no sense, you could get legit enum cases ending up in the `#[tlspl(other)]` catch-all case, which is obviously wrong.
  - a variable-length prefix that is not in its shortest form is rejected as `NonCanonicalLength` instead of being read as its value, so a vector has exactly one encoding.
  - containers reject a declared length that disagrees with what was consumed; zero-sized element types error instead of hanging (`Vec<()>` is a good example).
  - a known `#[tlspl(extensible)]` variant with leftover bytes in its buffer returns `TrailingBytes`.
  - `bool` and `Option` accept only 0/1;
  - duplicate map keys error instead of silently last-wins.
  - encoding into a short sink returns `Err(WriteZero)` where it previously returned `Ok(short)` with the payload truncated.
- Signed `#[repr]` enums are rejected at parse with a diagnostic citing RFC 8446 §3.3/§3.5, instead of failing at const evaluation.
- `#[tlspl(extensible)]` enum serialization is **1.5x to 1.65x faster**: the derive no longer buffers each variant body into a fresh `Vec` and copies it out.
  - Measured on 1000-element messages: 97.2 µs vs 148.7 µs at 4 KiB per element, 4.3 µs vs 7.2 µs at 40 B.
- The errors got compactified to help with stack usage on small return types (eg `ReadResult<u8>` which was dominated by the error type by a 40x factor).
  - `TlsplReadError` went from 40 bytes to 24 (-40%)
  - `TlsplWriteError` went from 40 bytes to 16 (-60%)
  - **Breaking**: The `Custom` case in both is now `Box`ed, so keep that in mind for `match`ing on the error
- **Breaking**: `parsio` has been updated to `0.1.x` series
  - If you ever built a custom `Read` implementation against the reexports in `thalassa::io`, you'll need to implement the two new trait methods (`is_eof` and `remaining`)
