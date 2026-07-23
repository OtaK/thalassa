# thalassa-derive

[![Crates.io](https://img.shields.io/crates/v/thalassa-derive.svg)](https://crates.io/crates/thalassa-derive)
[![docs.rs](https://docs.rs/thalassa-derive/badge.svg)](https://docs.rs/thalassa-derive)

## Description

Derives for [thalassa](https://crates.io/crates/thalassa)

Note: A lot of this is similar to tls_codec's derives, in an effort to ease migration between the derives/attrs

## Attributes

### On Enums

- `#[tlspl(untagged)]` - Marks that this enum contents should not be de/serialized preceded by their discriminant. This is especially useful when using the `#[tlspl(select)]` attribute
- `#[tlspl(extensible)]` - This marks an enum as "extensible", meaning that the contents of the variants (eg. its fields) will be serialized as a variable-length bytes container, to ensure that it can be extended, serialized and deserialized even in the case of future evolutions or downstream-defined extensions. This is inspired by how MLS (RFC9420) Extensions are done.

### On Enum Variants

- `#[tlspl(discrmininant = "path::to::constant"|1337)]` - Allows to point to a discriminant tag for cases where it's not supported by the Rust language (eg complex data types/structs where explicit discriminant isn't allowed).
  - This also supports the following forms
    - integer literals: `#[tlspl(discriminant = 69)]`
    - constants: `#[tlspl(discriminant = "MY_CONST")]` - note: wrapped in a string
    - enum variants that have an explicit discriminant: `#[tlspl(discriminant = "MyEnum::CaseN")]` - note: wrapped in a string
- `#[tlspl(other)]` - Marks a "catch-all" or "unknown" variant that needs to be a tuple having 1 field equal to the #[repr] of the enum (for naked enums), or 2 fields equal to `(repr, Cow<[u8]>)` (for enums that have data)
  - Example

```rust,ignore
#[derive(TlsplSize, TlsplDeserialize, TlsplSerialize)]
#[repr(u8)]
enum Thing {
    CaseA = 0,
    CaseB = 1,
    #[tlspl(other)]
    Unknown(u8)
}

#[derive(TlsplSize, TlsplDeserialize, TlsplSerialize)]
#[tlspl(extensible)]
#[repr(u8)]
enum ThingWithData<'a> {
    #[tlspl(discriminant = "Thing::CaseA")]
    CaseA {
        name: std::borrow::Cow<'a, str>,
    },
    #[tlspl(discriminant = "Thing::CaseB")]
    CaseB {
        flag: bool
    },
    #[tlspl(other)]
    Unknown(u8, std::borrow::Cow<'a, [u8]>),
}
```

### On Fields

- `#[tlspl(skip)]` - Skips this field during both serialization and deserialization. Requires `Default` to be implemented on the field since this data format has a fixed data layout.
- `#[tlspl(with = path::to::module)]` - Allows to override the ser/deser implementation of the underlying type with a custom implementation. The module should export any of those functions, depending on which trait your're deriving:
  - `tlspl_serialized_len(&T) -> usize`
  - `tlspl_serialize_to(&T, writer) -> TlsplWriteResult<usize>`
  - `tlspl_deserialize_from(reader) -> TlsplReadResult<T>`
- `#[tlspl(select = field.member.thing)]` - mirrors the `select` keyword in TLSPL prose found in specifications.
  - Restrictions
     1. The targeted field must be declared *BEFORE* this field. The order of declaration matters in TLSPL.
     2. The type of the field with this attribute must be an enum that has `#[tlspl(untagged)]`
  - Example

```rust,ignore
#[derive(TlsplAll)]
#[repr(u8)]
pub enum FieldDiscriminant {
    Variant1 = 0x01,
    Variant2 = 0x02,
}

#[derive(TlsplAll)]
#[tlspl(untagged)]
pub enum FieldContents {
    #[tlspl(discriminant = "FieldDiscriminant::Variant1")]
    Variant1,
    #[tlspl(discriminant = "FieldDiscriminant::Variant2")]
    Variant2 {
        thing: bool,
    }
}

#[derive(TlsplAll)]
pub struct ComplexStructure<'a> {
    pub field_type: FieldDiscriminant,
    pub unrelated_field: u64,
    pub another_field: Cow<'a, [u8]>,
    #[tlspl(select = field_type)]
    pub field_contents: FieldContents,
}
```

- Enum Variants
