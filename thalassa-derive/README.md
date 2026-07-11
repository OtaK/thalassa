# thalassa-derive

[![Crates.io](https://img.shields.io/crates/v/thalassa-derive.svg)](https://crates.io/crates/thalassa-derive)
[![docs.rs](https://docs.rs/thalassa-derive/badge.svg)](https://docs.rs/thalassa-derive)

## Description

Derives for [thalassa](https://crates.io/crates/thalassa)

Note: A lot of this is similar to tls_codec's derives, in an effort to ease migration between the derives/attrs

## Attributes

- Fields
  - `#[tlspl(skip)]` - Skips this field during both serialization and deserialization. Requires `Default` to be implemented on the field since this data format has a fixed data layout.
  - `#[tlspl(with = path::to::module)]` - Allows to override the ser/deser implementation of the underlying type with a custom implementation. The module should export any of those functions, depending on which trait your're deriving:
    - `tlspl_serialized_len(&T) -> usize`
    - `tlspl_serialize_to(&T, writer) -> TlsplWriteResult<usize>`
    - `tlspl_deserialize_from(reader) -> TlsplReadResult<T>`
- Enum Variants
  - `#[tlspl(discrmininant = "path::to::constant"|1337)]` - Allows to point to a discriminant tag for cases where it's not supported by the Rust language (eg complex data types/structs where explicit discriminant isn't allowed).
    - This also supports the following forms
      - integer literals: `#[tlspl(discriminant = 69)]`
      - constants: `#[tlspl(discriminant = "MY_CONST")]` - note: wrapped in a string
      - enum variants that have an explicit discriminant: `#[tlspl(discriminant = "MyEnum::CaseN")]` - note: wrapped in a string
