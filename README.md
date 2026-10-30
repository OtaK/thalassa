# Thalassa

[![Crates.io](https://img.shields.io/crates/v/thalassa.svg)](https://crates.io/crates/thalassa)
[![docs.rs](https://docs.rs/thalassa/badge.svg)](https://docs.rs/thalassa)

## Description

Implementation of the TLS presentation language data format defined in RFC8446.
Mostly cares about what MLS (RFC9420) cares about, eventually we'll cover the entirety of the spec to support other TLS-PL usecases (like, uh, TLS).

The killer feature compared to other crates is zero-copy deserialization, making it seriously fast, like up to 5TB/s fast on 1MB payloads.

This includes a derive proc macro heavily inspired by [tls_codec](https://crates.io/crates/tls_codec), with a feature focus on `discriminant` related attributes - which I contributed to.

Compatible with WASM.

## Features

- `derive`: `#[derive(TlsplSize, TlsplSerialize, TlsplDeserialize)]` (or `TlsplAll`).
- `futures-codec`: `thalassa::codec::TlsplCodec<T>`, an [`asynchronous-codec`](https://crates.io/crates/asynchronous-codec) `Decoder`/`Encoder`. Wrap any futures-io `AsyncRead`/`AsyncWrite` in `Framed::new(io, TlsplCodec::<T>::new(max_frame_len))` to get a `Stream` of `T` and a `Sink` of `T` (`TlsplCodec<&U>` encodes by reference, borrowed types included). Decoded values are owned (copied out of the read buffer).

## Documentation

Here: [https://docs.rs/thalassa](https://docs.rs/thalassa)

## Roadmap

- [x] Make E2E interop tests with `tls_codec`
- [x] Actually manage to benchmark the real performance? Despite my best efforts, I get numbers in the range of 500TB/s on deserialization, which absolutely doesn't sound right.
  - see [BENCHMARKS](benchmarks/BENCHMARKS.md)
- [x] "Distant variant" feature on the derive
  - This basically allows to de/serialize Rust enums as TLSPL variants, but allowing to use a discriminant *not* immediately preceding the variant contents, which is currently a requirement (as with `tls_codec`). This is *extremely* common in protocols making use of TLSPL, such as MLS, MIMI or keytrans.
  - Done! It's been named `#[tlspl(select)]` and will be available in 0.0.2
- [x] Billion Laughs protection (recursion depth tracking) - Done in 0.1.0
  - Unfortunately that cost a breaking change (`TlsplDeserialize` now takes a `parsio::DepthAwareReaderGuard<parsio::Read>` instead of a raw `parsio::Read`). It is what it is.
- [x] Serde compat - No need. Your types can derive serde to be used in another format. TLSPL is too problematic to be used with serde without massive hacks (like thread local fences and stuff like that).
- [x] Facet compat (maybe?) - NOPE. Not happening
- [ ] Docs improvements, examples, etc

## AI Disclaimer

Unlike a lot of things being created currently, the code of this library was written WITHOUT the use of any LLM. Anything under /src was 100% hand-written.

Some of the tests, on the other hand, were LLM-assisted, because I can't be bothered to write dozens of sample inputs.

## Acknowledgements

- [tls_codec](https://crates.io/crates/tls_codec) for the derive inspiration

## License

Licensed under either of these:

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
  [https://www.apache.org/licenses/LICENSE-2.0](https://www.apache.org/licenses/LICENSE-2.0))
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  [https://opensource.org/licenses/MIT](https://opensource.org/licenses/MIT))
