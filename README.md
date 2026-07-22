# Thalassa

[![Crates.io](https://img.shields.io/crates/v/thalassa.svg)](https://crates.io/crates/thalassa)
[![docs.rs](https://docs.rs/thalassa/badge.svg)](https://docs.rs/thalassa)

## Description

Implementation of the TLS presentation language data format defined in RFC8446.
Mostly cares about what MLS (RFC9420) cares about, eventually we'll cover the entirety of the spec to support other TLS-PL usecases (like, uh, TLS).

The killer feature compared to other crates is zero-copy deserialization, making it seriously fast, like up to 5TB/s fast on 1MB payloads.

This includes a derive proc macro heavily inspired by [tls_codec](https://crates.io/crates/tls_codec), with a feature focus on `discriminant` related attributes - which I contributed to.

Compatible with WASM.

## Documentation

Here: [https://docs.rs/thalassa](https://docs.rs/thalassa)

## Roadmap

- [x] Make E2E interop tests with `tls_codec`
- [x] Actually manage to benchmark the real performance? Despite my best efforts, I get numbers in the range of 500TB/s on deserialization, which absolutely doesn't sound right.
  - see [BENCHMARKS](benchmarks/BENCHMARKS.md)
- [x] "Distant variant" feature on the derive
  - This basically allows to de/serialize Rust enums as TLSPL variants, but allowing to use a discriminant *not* immediately preceding the variant contents, which is currently a requirement (as with `tls_codec`). This is *extremely* common in protocols making use of TLSPL, such as MLS, MIMI or keytrans.
  - Done! It's been named `#[tlspl(select)]` and will be available in 0.0.2
- [ ] Billion Laughs protection (recursion depth tracking)
- [ ] Serde compat
- [x] Facet compat (maybe?) - NOPE. Not happening
- [ ] Docs improvements, examples, etc

## AI Disclaimer

Unlike a lot of things being created currently, this library was written WITHOUT the use of any LLM.

## Acknowledgements

- [tls_codec](https://crates.io/crates/tls_codec) for the derive inspiration

## License

Licensed under either of these:

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
  [https://www.apache.org/licenses/LICENSE-2.0](https://www.apache.org/licenses/LICENSE-2.0))
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  [https://opensource.org/licenses/MIT](https://opensource.org/licenses/MIT))
