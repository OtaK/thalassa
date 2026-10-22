#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

// Note: A lot of this is similar to tls_codec's derives, in an effort to ease migration between the derives/attrs

use darling::{
    FromField, FromMeta, FromVariant as _,
    ast::Fields,
    util::{Flag, SpannedValue},
};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, quote_spanned};
use syn::{
    Attribute, DeriveInput, Expr, ExprLit, ExprPath, GenericParam, Generics, Ident, Lit, Type,
    parse_macro_input, parse_quote, spanned::Spanned,
};

#[derive(Debug, Clone)]
enum DiscriminantTarget {
    Lit(usize),
    VariantPath(ExprPath),
    Value(Expr),
}

impl darling::FromMeta for DiscriminantTarget {
    fn from_value(lit: &Lit) -> darling::Result<Self> {
        Ok(match lit {
            Lit::Str(lit_str) => Self::VariantPath(lit_str.parse()?),
            Lit::Byte(lit_byte) => Self::Lit(lit_byte.value() as _),
            Lit::Int(lit_int) => Self::Lit(lit_int.base10_parse()?),
            _ => {
                return Err(darling::Error::custom(
                    "expected: \
                      - quoted path to an enum variant: `\"path::to::Enum::Variant\"`\
                      - unquoted expression: const `MY_CONST`, integer or byte literal (`b'y'` / `69` (nice))",
                ).with_span(lit));
            }
        })
    }

    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        match expr {
            Expr::Group(group) => Self::from_expr(&group.expr),
            Expr::Lit(ExprLit { lit, .. }) => Self::from_value(lit),
            other => Ok(Self::Value(other.clone())),
        }
    }
}

#[derive(Debug, Clone)]
enum DiscriminantExpr {
    Lit(usize),
    Verbatim(Expr),
}

impl darling::FromMeta for DiscriminantExpr {
    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        Ok(match expr {
            Expr::Lit(ExprLit {
                lit: Lit::Byte(lit_byte),
                ..
            }) => Self::Lit(lit_byte.value() as usize),
            Expr::Lit(ExprLit {
                lit: Lit::Int(lit_int),
                ..
            }) => Self::Lit(lit_int.base10_parse::<usize>()?),
            _ => Self::Verbatim(expr.clone()),
        })
    }
}

#[derive(Debug, darling::FromMeta)]
struct TlsplEnumContainerAttrs {
    /// Marks that this enum contents should not be de/serialized preceded by their
    /// discriminant. This is especially useful when using the `#[tlspl(select)]` attribute
    untagged: Flag,
    /// This marks an enum as "extensible", meaning that the contents of the variants (eg. its fields) will be serialized
    /// as a variable-length bytes container, to ensure that it can be extended, serialized and deserialized
    /// even in the case of future evolutions or downstream-defined extensions. This is inspired by how MLS (RFC9420)
    /// Extensions are done.
    extensible: Flag,
}

#[derive(Debug, Clone, darling::FromMeta)]
struct TlsplFieldAttrs {
    /// Custom serialization pointing to a module that has any of those 3 functions:
    /// - tlspl_serialized_len(&T) -> usize
    /// - tlspl_serialize_to(&T, writer) -> WriteResult<usize>
    /// - tlspl_deserialize_from(reader) -> ReadResult<T>
    with: Option<ExprPath>,
    /// Skip this field (requires [`Default`] since serialization has fixed layout)
    skip: Flag,
    /// A derive feature made to ease the implementation of TLSPL structures
    /// that look like the code below (example section cont.). This mirrors the
    /// `select` keyword in TLSPL prose found in specifications.
    ///
    /// ### Restrictions
    ///
    /// 1. The targeted field must be declared *BEFORE* this field. The order of declaration matters in TLSPL.
    /// 2. The type of the field with this attribute must be an enum that has `#[tlspl(untagged)]`
    /// 3. The targeted field MUST implement `Copy`. You can also provide `select = field.clone()` to bypass this, but this isn't recommended
    ///
    /// ### Example
    ///
    /// ```rust
    /// # use thalassa_derive::*;
    /// #[derive(Copy, Clone, TlsplAll)]
    /// #[repr(u8)]
    /// pub enum FieldDiscriminant {
    ///     Variant1 = 0x01,
    ///     Variant2 = 0x02,
    /// }
    ///
    /// #[derive(TlsplAll)]
    /// #[repr(u8)]
    /// #[tlspl(untagged)]
    /// pub enum FieldContents {
    ///     #[tlspl(discriminant = "FieldDiscriminant::Variant1")]
    ///     Variant1,
    ///     #[tlspl(discriminant = "FieldDiscriminant::Variant2")]
    ///     Variant2 {
    ///         thing: bool,
    ///     }
    /// }
    ///
    /// #[derive(TlsplAll)]
    /// pub struct ComplexStructure<'a> {
    ///     pub field_type: FieldDiscriminant,
    ///     pub unrelated_field: u64,
    ///     pub another_field: std::borrow::Cow<'a, [u8]>,
    ///     #[tlspl(select = self.field_type)]
    ///     pub field_contents: FieldContents,
    /// }
    /// ```
    #[darling(default, map = Some)]
    select: Option<Expr>,
}

#[derive(Debug, darling::FromMeta)]
struct TlsplEnumVariantAttrs {
    /// Custom enum discriminant
    discriminant: Option<DiscriminantTarget>,
    /// Marks a "catch-all" or "unknown" variant that needs to
    /// contain a single tuple field equal to the #[repr] of the enum
    ///
    /// Eg: for a `#[repr(u8)]`, this variant needs to contain a `u8` like so:
    ///
    /// ```rust
    /// # use thalassa_derive::*;
    /// #[derive(TlsplSize, TlsplDeserialize, TlsplSerialize)]
    /// #[repr(u8)]
    /// enum Thing {
    ///     CaseA = 0,
    ///     CaseB = 1,
    ///     #[tlspl(other)]
    ///     Unknown(u8)
    /// }
    ///
    /// #[derive(TlsplSize, TlsplDeserialize, TlsplSerialize)]
    /// #[tlspl(extensible)]
    /// #[repr(u8)]
    /// enum ThingWithData<'a> {
    ///     #[tlspl(discriminant = "Thing::CaseA")]
    ///     CaseA {
    ///         name: std::borrow::Cow<'a, str>,
    ///     },
    ///     #[tlspl(discriminant = "Thing::CaseB")]
    ///     CaseB {
    ///         flag: bool
    ///     },
    ///     #[tlspl(other)]
    ///     Unknown(u8, std::borrow::Cow<'a, [u8]>),
    /// }
    /// ```
    other: Flag,
}

fn discr_const_ident(variant_ident: &Ident) -> Ident {
    quote::format_ident!("__THALASSA_{}", variant_ident)
}

#[derive(Debug, darling::FromField)]
#[darling(attributes(tlspl))]
struct FieldNamed {
    #[darling(with = darling::util::require_ident)]
    ident: Ident,
    ty: Type,
    #[darling(flatten)]
    attr: TlsplFieldAttrs,
}

#[derive(Debug)]
struct StructNamedFields {
    fields: Vec<SpannedValue<FieldNamed>>,
}

impl TryFrom<&syn::Data> for StructNamedFields {
    type Error = darling::Error;

    fn try_from(data: &syn::Data) -> std::prelude::v1::Result<Self, Self::Error> {
        let mut errors = darling::Error::accumulator();
        let syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(fields),
            ..
        }) = data
        else {
            unreachable!()
        };

        let fields = fields
            .named
            .iter()
            .filter_map(|field| errors.handle(SpannedValue::<FieldNamed>::from_field(field)))
            .collect();
        errors.finish_with(Self { fields })
    }
}

#[derive(Debug, darling::FromField)]
#[darling(attributes(tlspl))]
struct FieldTuple {
    ty: Type,
    #[darling(flatten)]
    attr: TlsplFieldAttrs,
}

#[derive(Debug)]
struct StructTupleFields {
    fields: Vec<SpannedValue<FieldTuple>>,
}

impl TryFrom<&syn::Data> for StructTupleFields {
    type Error = darling::Error;

    fn try_from(data: &syn::Data) -> std::prelude::v1::Result<Self, Self::Error> {
        let mut errors = darling::Error::accumulator();
        let syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Unnamed(fields),
            ..
        }) = data
        else {
            unreachable!()
        };

        let fields = fields
            .unnamed
            .iter()
            .filter_map(|field| errors.handle(SpannedValue::<FieldTuple>::from_field(field)))
            .collect();
        errors.finish_with(Self { fields })
    }
}

#[derive(Debug)]
enum VariantDiscriminant {
    ViaAttr {
        target: DiscriminantTarget,
        native: Option<DiscriminantExpr>,
    },
    Explicit(DiscriminantExpr),
    Implicit,
}

#[derive(Debug, darling::FromVariant)]
#[darling(attributes(tlspl))]
struct VariantUnit {
    ident: Ident,
    discriminant: Option<Expr>,
    #[darling(flatten)]
    attr: TlsplEnumVariantAttrs,
}

impl VariantUnit {
    fn discr(&self, span: &Span) -> darling::Result<VariantDiscriminant> {
        let explicit_discr = if let Some(expr) = &self.discriminant {
            Some(DiscriminantExpr::from_expr(expr)?)
        } else {
            None
        };

        if let Some(target) = &self.attr.discriminant {
            // Small sanity checks
            if let (
                Some(DiscriminantExpr::Lit(native_discr)),
                DiscriminantTarget::Lit(custom_discr),
            ) = (&explicit_discr, target)
                && native_discr != custom_discr
            {
                return Err(darling::Error::custom("The explicit and custom discriminants are mismatched, this would result in weird errors").with_span(span));
            }

            return Ok(VariantDiscriminant::ViaAttr {
                target: target.clone(),
                native: explicit_discr,
            });
        }

        if let Some(discr) = explicit_discr {
            return Ok(VariantDiscriminant::Explicit(discr));
        }

        Ok(VariantDiscriminant::Implicit)
    }
}

#[derive(Debug, darling::FromVariant)]
#[darling(attributes(tlspl))]
struct VariantTuple {
    ident: Ident,
    discriminant: Option<Expr>,
    fields: Fields<SpannedValue<FieldTuple>>,
    #[darling(flatten)]
    attr: TlsplEnumVariantAttrs,
}

impl VariantTuple {
    fn discr(&self, span: &Span) -> darling::Result<VariantDiscriminant> {
        let explicit_discr = if let Some(expr) = &self.discriminant {
            Some(DiscriminantExpr::from_expr(expr)?)
        } else {
            None
        };

        if let Some(target) = &self.attr.discriminant {
            // Small sanity check
            if let (
                Some(DiscriminantExpr::Lit(native_discr)),
                DiscriminantTarget::Lit(custom_discr),
            ) = (&explicit_discr, target)
                && native_discr != custom_discr
            {
                return Err(darling::Error::custom("The explicit and custom discriminants are mismatched, this would result in weird errors").with_span(span));
            }

            return Ok(VariantDiscriminant::ViaAttr {
                target: target.clone(),
                native: explicit_discr,
            });
        }

        if let Some(discr) = explicit_discr {
            return Ok(VariantDiscriminant::Explicit(discr));
        }

        Ok(VariantDiscriminant::Implicit)
    }
}

#[derive(Debug, darling::FromVariant)]
#[darling(attributes(tlspl))]
struct VariantNamed {
    ident: Ident,
    discriminant: Option<Expr>,
    fields: Fields<SpannedValue<FieldNamed>>,
    #[darling(flatten)]
    attr: TlsplEnumVariantAttrs,
}

impl VariantNamed {
    fn discr(&self, span: &Span) -> darling::Result<VariantDiscriminant> {
        let explicit_discr = if let Some(expr) = &self.discriminant {
            Some(DiscriminantExpr::from_expr(expr)?)
        } else {
            None
        };

        if let Some(target) = &self.attr.discriminant {
            // Small sanity check
            if let (
                Some(DiscriminantExpr::Lit(native_discr)),
                DiscriminantTarget::Lit(custom_discr),
            ) = (&explicit_discr, target)
                && native_discr != custom_discr
            {
                return Err(darling::Error::custom("The explicit and custom discriminants are mismatched, this would result in weird errors").with_span(span));
            }

            return Ok(VariantDiscriminant::ViaAttr {
                target: target.clone(),
                native: explicit_discr,
            });
        }

        if let Some(discr) = explicit_discr {
            return Ok(VariantDiscriminant::Explicit(discr));
        }

        Ok(VariantDiscriminant::Implicit)
    }
}

#[derive(Debug)]
struct MemberWithAttrs {
    span: Span,
    ident: Ident,
    ty: Type,
    attrs: TlsplFieldAttrs,
}

#[derive(Debug)]
enum Variant {
    Unit(SpannedValue<VariantUnit>),
    Named(SpannedValue<VariantNamed>),
    Tuple(SpannedValue<VariantTuple>),
}

impl Variant {
    fn span(&self) -> Span {
        match self {
            Variant::Unit(variant_unit) => variant_unit.span(),
            Variant::Named(variant_named) => variant_named.span(),
            Variant::Tuple(variant_tuple) => variant_tuple.span(),
        }
    }

    fn ident(&self) -> &Ident {
        match self {
            Variant::Unit(variant_unit) => &variant_unit.ident,
            Variant::Named(variant_named) => &variant_named.ident,
            Variant::Tuple(variant_tuple) => &variant_tuple.ident,
        }
    }

    fn discriminant(&self) -> darling::Result<VariantDiscriminant> {
        match self {
            Variant::Unit(variant_unit) => variant_unit.discr(&variant_unit.span()),
            Variant::Named(variant_named) => variant_named.discr(&variant_named.span()),
            Variant::Tuple(variant_tuple) => variant_tuple.discr(&variant_tuple.span()),
        }
    }

    fn attrs(&self) -> &TlsplEnumVariantAttrs {
        match self {
            Variant::Unit(variant_unit) => &variant_unit.attr,
            Variant::Named(variant_named) => &variant_named.attr,
            Variant::Tuple(variant_tuple) => &variant_tuple.attr,
        }
    }

    fn members_with_exprs(&self) -> Box<dyn Iterator<Item = MemberWithAttrs> + '_> {
        match self {
            Variant::Unit(_) => Box::new(std::iter::empty()),
            Variant::Named(variant_named) => {
                Box::new(variant_named.fields.iter().map(|f| MemberWithAttrs {
                    span: f.span(),
                    ident: f.ident.clone(),
                    ty: f.ty.clone(),
                    attrs: f.attr.clone(),
                }))
            }
            Variant::Tuple(variant_tuple) => {
                Box::new(variant_tuple.fields.iter().enumerate().map(|(idx, f)| {
                    let span = f.span();
                    MemberWithAttrs {
                        ident: format_ident!("tuple{}", idx, span = f.span()),
                        span,
                        ty: f.ty.clone(),
                        attrs: f.attr.clone(),
                    }
                }))
            }
        }
    }

    fn field_count(&self) -> usize {
        match self {
            Variant::Unit(_) => 0,
            Variant::Named(variant_named) => variant_named.fields.len(),
            Variant::Tuple(variant_tuple) => variant_tuple.fields.len(),
        }
    }
}

impl TryFrom<&syn::Variant> for Variant {
    type Error = darling::Error;

    fn try_from(variant: &syn::Variant) -> std::prelude::v1::Result<Self, Self::Error> {
        Ok(match variant.fields {
            syn::Fields::Named(_) => {
                Self::Named(SpannedValue::<VariantNamed>::from_variant(variant)?)
            }
            syn::Fields::Unnamed(_) => {
                Self::Tuple(SpannedValue::<VariantTuple>::from_variant(variant)?)
            }
            syn::Fields::Unit => Self::Unit(SpannedValue::<VariantUnit>::from_variant(variant)?),
        })
    }
}

#[derive(Debug)]
struct EnumVariants {
    variants: Vec<Variant>,
}

impl EnumVariants {
    fn discriminant_consts(&self, enum_target: &EnumTarget) -> darling::Result<TokenStream2> {
        let enum_ident = enum_target.ident.clone();
        let repr = extract_repr_from_attrs(&enum_target.attrs)?;

        let enum_is_naked = self.variants.iter().all(|v| matches!(v, Variant::Unit(_)));
        let spans = if enum_is_naked {
            self.variants
                .iter()
                .map(|v| {
                    if v.attrs().discriminant.is_some() {
                        return Err(darling::Error::custom(
                            r#"This is a naked enum, you do not need to use the #[tlspl] attribute
to define discriminants, just set an explicit discriminant to your variants, like so

#[derive(thalassa::TlsplAll)]
#[repr(u8)]
enum MyEnum {
    Variant1 = 0x12,
    Variant2 = 0xAC,
}

"#,
                        ));
                    }
                    let variant_ident = v.ident();
                    let const_id = discr_const_ident(variant_ident);

                    Ok(quote! {
                        #[allow(non_upper_case_globals)]
                        const #const_id: #repr = #enum_ident::#variant_ident as #repr;
                    })
                })
                .collect::<darling::Result<Vec<_>>>()?
        } else {
            enum DiscrChain {
                Known(usize),
                Emitted(TokenStream2),
            }

            let mut discr_chain = DiscrChain::Known(0);
            let mut path_used = false;

            let mut spans = Vec::with_capacity(self.variants.len());
            for variant in &self.variants {
                // The `other` case doesn't need a discriminant const, as it's a fallback
                if variant.attrs().other.is_present() {
                    continue;
                }

                let variant_ident = variant.ident();
                let const_id = discr_const_ident(variant_ident);
                let span = variant.span();

                fn path_used_err(span: &Span) -> darling::Error {
                    darling::Error::custom(
                        r#"The `#[tlspl(discriminant = \"path::to::thing\")]` is missing.
It is a viral attribute, and if one variant uses a path discriminant, then ALL your variants must do so as well."#,
                    ).with_span(span)
                }

                let tokens = match variant.discriminant()? {
                    VariantDiscriminant::ViaAttr {
                        target: discriminant_target,
                        native,
                    } => {
                        let native_assert = match &native {
                            Some(DiscriminantExpr::Verbatim(expr)) => quote_spanned! { span=>
                                const _: () = assert!((#expr) as #repr == #const_id,
                                    concat!(
                                        "`", stringify!(#enum_ident), "::", stringify!(#variant_ident), "`: its native discriminant (`",
                                        stringify!(#expr), "`) and its `#[tlspl(discriminant = ...)]` disagree",
                                    ));
                            },
                            Some(DiscriminantExpr::Lit(value)) => quote_spanned! { span=>
                                const _: () = assert!((#value) as #repr == #const_id,
                                    concat!(
                                        "`", stringify!(#enum_ident), "::", stringify!(#variant_ident), "`: its native discriminant (`",
                                        stringify!(#value), "`) and its `#[tlspl(discriminant = ...)]` disagree",
                                    ));
                            },
                            _ => quote!(),
                        };

                        match discriminant_target {
                            DiscriminantTarget::Value(expr) => {
                                if path_used {
                                    return Err(path_used_err(&span));
                                }

                                discr_chain = DiscrChain::Emitted(quote!(#const_id));
                                quote_spanned! { span=>
                                    #[allow(non_upper_case_globals)]
                                    const #const_id: #repr = #expr;
                                    #native_assert
                                }
                            }
                            DiscriminantTarget::Lit(value) => {
                                if path_used {
                                    return Err(path_used_err(&span));
                                }

                                discr_chain = DiscrChain::Known(value + 1);
                                quote_spanned! { span=>
                                    #[allow(non_upper_case_globals)]
                                    const #const_id: #repr = {
                                        if #value < #repr::MIN as usize || #value > #repr::MAX as usize {
                                            panic!("enum repr overflow");
                                        }

                                        #value as #repr
                                    };
                                    #native_assert
                                }
                            }
                            DiscriminantTarget::VariantPath(expr_path) => {
                                path_used = true;
                                quote_spanned! { span=>
                                    #[allow(clippy::unnecessary_cast, non_upper_case_globals)]
                                    const #const_id: #repr = {
                                        #[deny(enum_intrinsics_non_enums)]
                                        let read_size = core::mem::size_of_val(&core::mem::discriminant(&#expr_path));
                                        // Sanity check guard, to prevent OoB reads
                                        assert!(
                                            read_size <= core::mem::size_of_val(&#expr_path),
                                            "the discriminant of this `#[tlspl(discriminant = ...)]` target is wider than the enum itself",
                                        );
                                        // SAFETY: The discriminant sits at offset 0 for the `#[repr(uint)]` enums this derive supports.
                                        // So we read like savages the discriminant, hinted by the `core::mem::discriminant` call above
                                        let expr_us: usize = match read_size {
                                            1 => unsafe { *((&#expr_path) as *const _ as *const u8) as usize },
                                            2 => unsafe { *((&#expr_path) as *const _ as *const u16) as usize },
                                            4 => unsafe { *((&#expr_path) as *const _ as *const u32) as usize },
                                            8 => unsafe { *((&#expr_path) as *const _ as *const u64) as usize },
                                            _ => panic!(
                                                "unsupported `#[tlspl(discriminant = ...)]` target: its enum repr is not a `u[8|16|32|64]`",
                                            ),
                                        };

                                        if expr_us < #repr::MIN as usize || expr_us > #repr::MAX as usize {
                                            panic!("this `#[tlspl(discriminant = ...)]` target overflows this enum's `repr`");
                                        }

                                        expr_us as #repr
                                    };
                                    #native_assert
                                }
                            }
                        }
                    }
                    VariantDiscriminant::Explicit(value) => {
                        if path_used {
                            return Err(path_used_err(&span));
                        }

                        match value {
                            DiscriminantExpr::Lit(value) => {
                                discr_chain = DiscrChain::Known(value + 1);
                                quote_spanned! { span=>
                                    #[allow(non_upper_case_globals)]
                                    const #const_id: #repr = {
                                        if #value < #repr::MIN as usize || #value > #repr::MAX as usize { panic!("enum repr overflow"); }
                                        #value as #repr
                                    };
                                }
                            }
                            DiscriminantExpr::Verbatim(expr) => {
                                discr_chain = DiscrChain::Emitted(quote!(#const_id));
                                quote_spanned! { span=>
                                    #[allow(clippy::unnecessary_cast, non_upper_case_globals)]
                                    const #const_id: #repr = (#expr) as #repr;
                                }
                            }
                        }
                    }
                    VariantDiscriminant::Implicit => {
                        if path_used {
                            return Err(path_used_err(&span));
                        }

                        match discr_chain {
                            DiscrChain::Known(v) => {
                                discr_chain = DiscrChain::Known(v + 1);
                                quote_spanned! { span=>
                                    #[allow(non_upper_case_globals)]
                                    const #const_id: #repr = {
                                        if #v > #repr::MAX as usize { panic!("enum repr overflow: too many variants"); }
                                        #v as #repr
                                    };
                                }
                            }
                            DiscrChain::Emitted(prev) => {
                                discr_chain = DiscrChain::Emitted(quote!(#const_id));
                                quote_spanned! { span=>
                                    #[allow(non_upper_case_globals)]
                                    const #const_id: #repr = {
                                        if #prev == #repr::MAX { panic!("enum repr overflow: too many variants"); }
                                        #prev + 1
                                    };
                                }
                            }
                        }
                    }
                };

                spans.push(tokens);
            }

            spans
        };

        Ok(quote! { #(#spans)* })
    }
}

impl TryFrom<&syn::Data> for EnumVariants {
    type Error = darling::Error;

    fn try_from(data: &syn::Data) -> std::prelude::v1::Result<Self, Self::Error> {
        let mut errors = darling::Error::accumulator();
        let syn::Data::Enum(variants) = data else {
            unreachable!()
        };

        let fields = variants
            .variants
            .iter()
            .filter_map(|field| errors.handle(Variant::try_from(field)))
            .collect();

        errors.finish_with(Self { variants: fields })
    }
}

#[derive(Debug, darling::FromDeriveInput)]
struct StructUnitTarget {
    ident: Ident,
    generics: Generics,
}

#[derive(Debug, darling::FromDeriveInput)]
struct StructNamedTarget {
    ident: Ident,
    generics: Generics,
    #[darling(with = TryFrom::try_from)]
    data: StructNamedFields,
}

#[derive(Debug, darling::FromDeriveInput)]
struct StructTupleTarget {
    ident: Ident,
    generics: Generics,
    #[darling(with = TryFrom::try_from)]
    data: StructTupleFields,
}

fn extract_repr_from_attrs(attrs: &[Attribute]) -> darling::Result<Ident> {
    attrs
        .iter()
        .find_map(|attr| {
            attr.path()
                .is_ident("repr")
                .then(|| attr.parse_args::<Ident>().map_err(Into::into))
        })
        .ok_or_else(|| {
            darling::Error::custom(
                "This enum doesn't have a #[repr] attribute, we can't do much with this",
            )
        })?
        .and_then(|repr| {
            if ["u8", "u16", "u32", "u64"].iter().any(|allowed| repr == allowed) {
                Ok(repr)
            } else if repr == "usize" || repr == "isize" { // pointer-size dependent things
                Err(darling::Error::custom(
                    "`usize` and `isize` `#[repr]`s are forbidden, as their byte representation depends on pointer size\
                    and would change depending on your compilation target"
                ).with_span(&repr.span()))
            } else { // All that's left is iXX
                Err(darling::Error::custom(format!(
                    "`#[repr({repr})]` is not a valid TLSPL enum repr per RFC8446 (sections 3.3 and 3.5)\
                    only `u8`, `u16`, `u32` and `u64` have a wire representation."
                )).with_span(&repr.span()))
            }
        })
}

#[derive(Debug, darling::FromDeriveInput)]
#[darling(attributes(tlspl), forward_attrs(repr))]
struct EnumTarget {
    ident: Ident,
    generics: Generics,
    attrs: Vec<Attribute>,
    #[darling(with = TryFrom::try_from)]
    data: EnumVariants,
    #[darling(flatten)]
    attr: TlsplEnumContainerAttrs,
}

#[derive(Debug)]
enum TlsplDeriveTarget {
    StructUnit(SpannedValue<StructUnitTarget>),
    StructNamed(SpannedValue<StructNamedTarget>),
    StructTuple(SpannedValue<StructTupleTarget>),
    Enum(SpannedValue<EnumTarget>),
}

impl TlsplDeriveTarget {
    #[allow(dead_code)]
    fn span(&self) -> Span {
        match self {
            TlsplDeriveTarget::StructUnit(struct_unit_target) => struct_unit_target.span(),
            TlsplDeriveTarget::StructNamed(struct_named_target) => struct_named_target.span(),
            TlsplDeriveTarget::StructTuple(struct_tuple_target) => struct_tuple_target.span(),
            TlsplDeriveTarget::Enum(enum_target) => enum_target.span(),
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct ImplTargets: u8 {
        const SIZE = 1 << 0;
        const SERIALIZE = 1 << 1;
        const DESERIALIZE = 1 << 2;

        const ALL = Self::SIZE.bits() | Self::SERIALIZE.bits() | Self::DESERIALIZE.bits();
    }
}

fn augment_generics_ty(generics: &Generics, impl_targets: ImplTargets) -> Generics {
    let mut generics = generics.clone();

    for ty in generics.type_params_mut() {
        if impl_targets.contains(ImplTargets::SIZE) {
            let tlspl_size_bound = syn::TypeParamBound::Trait(parse_quote!(thalassa::TlsplSize));
            if ty.bounds.iter().all(|bound| bound != &tlspl_size_bound) {
                ty.bounds.push(tlspl_size_bound);
            }
        }

        if impl_targets.contains(ImplTargets::SERIALIZE) {
            let tlspl_ser_bound =
                syn::TypeParamBound::Trait(parse_quote!(thalassa::TlsplSerialize));
            if ty.bounds.iter().all(|bound| bound != &tlspl_ser_bound) {
                ty.bounds.push(tlspl_ser_bound);
            }
        }

        if impl_targets.contains(ImplTargets::DESERIALIZE) {
            let tlspl_deser_bound =
                syn::TypeParamBound::Trait(parse_quote!(thalassa::TlsplDeserialize<'tlspl>));
            if ty.bounds.iter().all(|bound| bound != &tlspl_deser_bound) {
                ty.bounds.push(tlspl_deser_bound);
            }

            let tlspl_lt_bound = syn::TypeParamBound::Lifetime(parse_quote!('tlspl));
            if ty.bounds.iter().all(|bound| bound != &tlspl_lt_bound) {
                ty.bounds.push(tlspl_lt_bound);
            }
        }
    }

    generics
}

fn augment_generics_with_lt(generics: &Generics) -> Generics {
    let mut generics = generics.clone();

    if generics.lifetimes().next().is_none() {
        generics
            .params
            .push(GenericParam::Lifetime(parse_quote!('tlspl)));
    } else if generics.lifetimes().all(|lt| lt.lifetime.ident != "tlspl") {
        let lifetimes = generics.lifetimes();
        generics.params.push(GenericParam::Lifetime(
            parse_quote!('tlspl: #(#lifetimes)+*),
        ));
    }

    generics
}

impl darling::FromDeriveInput for TlsplDeriveTarget {
    fn from_derive_input(input: &DeriveInput) -> darling::Result<Self> {
        let span = input.span();
        Ok(match &input.data {
            syn::Data::Struct(syn::DataStruct {
                fields: syn::Fields::Named(_),
                ..
            }) => Self::StructNamed(SpannedValue::new(
                StructNamedTarget::from_derive_input(input)?,
                span,
            )),
            syn::Data::Struct(syn::DataStruct {
                fields: syn::Fields::Unnamed(_),
                ..
            }) => Self::StructTuple(SpannedValue::new(
                StructTupleTarget::from_derive_input(input)?,
                span,
            )),
            syn::Data::Struct(syn::DataStruct {
                fields: syn::Fields::Unit,
                ..
            }) => Self::StructUnit(SpannedValue::new(
                StructUnitTarget::from_derive_input(input)?,
                span,
            )),
            syn::Data::Enum(_) => {
                let enum_def = EnumTarget::from_derive_input(input)?;
                let enum_is_nakd = enum_def
                    .data
                    .variants
                    .iter()
                    // Skip over variants that are marked #[tlspl(other)] to get an accurate count
                    .filter(|v| !v.attrs().other.is_present())
                    .all(|v| v.field_count() == 0);
                let enum_has_catchall = enum_def
                    .data
                    .variants
                    .iter()
                    .find(|v| v.attrs().other.is_present());

                if enum_def.attr.untagged.is_present() {
                    let all_variants_have_discriminants =
                        enum_def.data.variants.iter().all(|variant| {
                            matches!(
                                variant.attrs().discriminant,
                                Some(DiscriminantTarget::VariantPath(_))
                            )
                        });

                    if !all_variants_have_discriminants {
                        return Err(darling::Error::custom(
                            "Untagged enums REQUIRE to have all of their custom discriminants pointing to paths (eg: `#[tlspl(discriminant = \"OtherEnum::Variant\")]`",
                        ));
                    }
                }

                if let Some(catchall_variant) = enum_has_catchall {
                    if !enum_is_nakd && !enum_def.attr.extensible.is_present() {
                        return Err(darling::Error::custom(
                            "`#[tlspl(other)]` is only useable on enums that are either naked enums (with no fields in variants) or enums that are marked `extensible`",
                        ));
                    }

                    let enum_repr = extract_repr_from_attrs(&enum_def.attrs)?;
                    let field_n = catchall_variant.field_count();

                    let first_ty_matches_repr = || {
                        if let Some(Type::Path(tp)) =
                            catchall_variant.members_with_exprs().next().map(|m| m.ty)
                        {
                            tp.path.get_ident().unwrap() == &enum_repr
                        } else {
                            false
                        }
                    };

                    let second_ty_is_bytes = || {
                        if let Some(Type::Path(tp)) =
                            &catchall_variant.members_with_exprs().nth(1).map(|m| m.ty)
                        {
                            tp.path.segments.last().unwrap().ident == "Cow"
                        } else {
                            false
                        }
                    };

                    if enum_is_nakd {
                        if field_n != 1 || !first_ty_matches_repr() {
                            return Err(darling::Error::custom(
                                "Catch-all variants marked with `#[tlspl(other)]` on naked enums need to have exactly *one* tuple field equal to the `#[repr]` of the enum",
                            ));
                        }
                    } else {
                        if field_n != 2 || !first_ty_matches_repr() || !second_ty_is_bytes() {
                            return Err(darling::Error::custom(
                                "Catch-all variants marked with `#[tlspl(other)]` on naked enums need to have exactly *two* tuple field equal to the `#[repr]` of the enum and a Cow<[u8]> container afterwards (like so: `Other(u8, Cow<'a, [u8]>)`",
                            ));
                        }
                    }
                }
                Self::Enum(SpannedValue::new(enum_def, span))
            }
            syn::Data::Union(_) => {
                return Err(darling::Error::custom(
                    "Unions are not supported in the TLSPL data scheme",
                ));
            }
        })
    }
}

fn discr_fence_name(enum_ident: &Ident) -> Ident {
    format_ident!("__THALASSA_DISCR_FENCE_{}", enum_ident)
}

fn validate_select_target(span: Span, ty: &Type) -> darling::Result<()> {
    if !matches!(ty, syn::Type::Path(_)) {
        return Err(darling::Error::custom(
            "`#[tlspl(select = ...)]` needs to target a field whose type is a `#[tlspl(untagged)]` enum",
        )
        .with_span(&span));
    }
    Ok(())
}

fn resolve_select_target(
    expr: &Expr,
    members: &[MemberWithAttrs],
    select_pos: usize,
    span: Span,
) -> darling::Result<(TokenStream2, Type)> {
    let mut chain: Vec<&syn::Member> = Vec::new();

    let mut cursor = expr;
    while let Expr::Field(field) = cursor {
        chain.push(&field.member);
        cursor = &field.base;
    }

    // Check if the first part is `self.`. If not, bail, because that's hard deprecated
    let base_is_self = matches!(cursor, Expr::Path(path) if path.path.is_ident("self"));
    if !base_is_self {
        return Err(darling::Error::custom(
            "`#[tlspl(select = <field>)]` is pre 0.1.0 and has been removed / hard deprecated. \
             use `self.<field>` or `self.<index>` (for tuples) instead.\
             \nExample: `#[tlspl(select = self.my_field.other.thing)]` or `#[tlspl(select = self.0)]` for tuples",
        )
        .with_span(&span));
    }

    chain.reverse();

    let first = chain.first().ok_or_else(|| {
        darling::Error::custom("`#[tlspl(select = self)]` does not name a member").with_span(&span)
    })?;

    let target_pos = match first {
        syn::Member::Named(ident) => members.iter().position(|m| m.ident == *ident),
        syn::Member::Unnamed(index) => Some(index.index as usize),
    }
    .filter(|pos| *pos < members.len())
    .ok_or_else(|| {
        darling::Error::custom(format!(
            "`#[tlspl(select = self.{})]` does not exist",
            quote::ToTokens::to_token_stream(first)
        ))
        .with_span(&span)
    })?;

    if target_pos >= select_pos {
        return Err(darling::Error::custom(
            "this `#[tlspl(select = ...)]`-ed field MUST be declared *after* the field it targets",
        )
        .with_span(&span));
    }

    let ident = members[target_pos].ident.clone();
    let ty = members[target_pos].ty.clone();
    let rest = &chain[1..];

    let tokens = quote_spanned! { span=> #ident #(.#rest)* };
    Ok((tokens, ty))
}

fn emit_select_fence_store(
    span: Span,
    ty: &Type,
    select_target: TokenStream2,
    select_target_ty: Type,
) -> TokenStream2 {
    quote_spanned! { span=>
        {
            const {
                assert!(
                    core::mem::size_of::<#select_target_ty>() <= core::mem::size_of::<<#ty as thalassa::util::TlsplUntaggedEnum>::Discriminant>(),
                    "the `#[tlspl(select = ...)]` target's repr is wider than this field's type's untagged `#[repr]`"
                );
            };

            let discr: <#ty as thalassa::util::TlsplUntaggedEnum>::Discriminant = #select_target as _;
            <#ty as thalassa::util::TlsplUntaggedEnum>::__thalassa_discr_fence().replace(Some(discr));
        }
    }
}

fn is_u8_type(ty: &syn::Type) -> bool {
    let syn::Type::Path(ty) = (match ty {
        // `macro_rules!` interpolation via `$t:ty` and verbatim `(u8)` both arrive wrapped
        // for some reason
        syn::Type::Group(group) => &group.elem,
        syn::Type::Paren(paren) => &paren.elem,
        other => other,
    }) else {
        return false;
    };

    ty.qself.is_none()
        && ty.path.segments.last().is_some_and(|seg| {
            seg.ident == "u8" && matches!(seg.arguments, syn::PathArguments::None)
        })
}

impl TlsplDeriveTarget {
    fn impl_size(&self) -> darling::Result<TokenStream2> {
        Ok(match self {
            TlsplDeriveTarget::StructNamed(sv) => {
                let span = sv.span();
                let StructNamedTarget {
                    ident,
                    generics,
                    data,
                } = sv.as_ref();
                let generics = augment_generics_ty(generics, ImplTargets::SIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();

                let member_calls = data.fields.iter().filter_map(|f| {
                    if f.attr.skip.is_present() {
                        return None;
                    }

                    let ident = syn::Member::Named(f.ident.clone());
                    Some(if let Some(module_with) = &f.attr.with {
                        quote_spanned! {f.span()=> #module_with::tlspl_serialized_len(&self.#ident) }
                    } else if is_u8_type(&f.ty) {
                        quote_spanned! {f.span()=> 1 }
                    } else {
                        quote_spanned! {f.span()=> thalassa::TlsplSize::tlspl_serialized_len(&self.#ident) }
                    })
                });

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSize for #ident #ty_generics #where_c {
                        #[inline]
                        #[allow(clippy::identity_op)]
                        fn tlspl_serialized_len(&self) -> usize {
                            0 #(+ #member_calls)*
                        }
                    }
                }
            }
            TlsplDeriveTarget::StructTuple(sv) => {
                let span = sv.span();
                let StructTupleTarget {
                    ident,
                    generics,
                    data,
                } = sv.as_ref();
                let generics = augment_generics_ty(generics, ImplTargets::SIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();

                let member_calls = data.fields.iter().enumerate().filter_map(|(idx, f)| {
                    if f.attr.skip.is_present() {
                        return None;
                    }

                    let ident = syn::Member::Unnamed(idx.into());
                    Some(if let Some(module_with) = &f.attr.with {
                        quote_spanned! { f.span()=> #module_with::tlspl_serialized_len(&self.#ident) }
                    } else if is_u8_type(&f.ty) {
                        quote_spanned! {f.span()=> 1 }
                    } else {
                        quote_spanned! { f.span()=> thalassa::TlsplSize::tlspl_serialized_len(&self.#ident) }
                    })
                });

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSize for #ident #ty_generics #where_c {
                        #[inline]
                        #[allow(clippy::identity_op)]
                        fn tlspl_serialized_len(&self) -> usize {
                            0 #(+ #member_calls)*
                        }
                    }
                }
            }
            TlsplDeriveTarget::StructUnit(sv) => {
                let span = sv.span();
                let StructUnitTarget { ident, generics } = sv.as_ref();
                let generics = augment_generics_ty(generics, ImplTargets::SIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSize for #ident #ty_generics #where_c {
                        #[inline]
                        #[allow(clippy::identity_op)]
                        fn tlspl_serialized_len(&self) -> usize { 0 }
                    }
                }
            }
            Self::Enum(sv) => {
                let span = sv.span();
                let EnumTarget {
                    ident,
                    generics,
                    attrs,
                    data,
                    attr,
                } = sv.as_ref();
                let repr = extract_repr_from_attrs(attrs)?;
                let generics = augment_generics_ty(generics, ImplTargets::SIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();

                let variant_arms = data.variants.iter().map(|variant| {
                    let span = variant.span();
                    let variant_ident = variant.ident();
                    let variant_is_tuple = matches!(variant, Variant::Tuple(_));

                    let (field_mappings, field_mapping_calls): (Vec<_>, Vec<_>) = variant.members_with_exprs().filter_map(|member| {
                        if member.attrs.skip.is_present() {
                            return None;
                        }

                        let ident = member.ident;
                        let method_mapping = if let Some(with) = member.attrs.with {
                            quote_spanned! { member.span=> #with::tlspl_serialized_len(&#ident) }
                        } else if is_u8_type(&member.ty) {
                            quote_spanned! { member.span=> 1 }
                        } else {
                            quote_spanned! { member.span=> thalassa::TlsplSize::tlspl_serialized_len(&#ident) }
                        };

                        Some((ident, method_mapping))
                    }).unzip();

                    if variant.attrs().other.is_present() {
                        return quote_spanned! { span=>
                            #ident::#variant_ident(#(#field_mappings,)* ..) => { return 0 #(+ #field_mapping_calls)*; },
                        };
                    }

                    if variant_is_tuple {
                        quote_spanned! { span=>
                            #ident::#variant_ident(#(#field_mappings,)* ..) => 0 #(+ #field_mapping_calls)*,
                        }
                    } else {
                        quote_spanned! { span=>
                            #ident::#variant_ident { #(#field_mappings,)* .. } => 0 #(+ #field_mapping_calls)*,
                        }
                    }
                });

                let (discr_block, untagged_trait_impl, untagged_discr_fence) = if attr
                    .untagged
                    .is_present()
                {
                    let discr_fence_name = discr_fence_name(ident);
                    (
                        quote!(0),
                        quote! {

                            #[automatically_derived]
                            unsafe impl #impl_generics thalassa::util::TlsplUntaggedEnum for #ident #ty_generics #where_c {
                                type Discriminant = #repr;
                                #[inline]
                                fn __thalassa_discr_fence() -> &'static std::thread::LocalKey<std::cell::Cell<Option<#repr>>> {
                                    &#discr_fence_name
                                }
                            }
                        },
                        quote! {

                            std::thread_local! {
                                #[allow(non_upper_case_globals, clippy::identity_op)]
                                pub(crate) static #discr_fence_name: std::cell::Cell<Option<#repr>> = const { std::cell::Cell::new(None) };
                            }
                        },
                    )
                } else {
                    (quote!(core::mem::size_of::<#repr>()), quote!(), quote!())
                };

                let maybe_extensible = if attr.extensible.is_present() {
                    quote!(thalassa::types::content_len_as_vlbytes_overhead(
                        content_len
                    ))
                } else {
                    quote!(0)
                };

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSize for #ident #ty_generics #where_c {
                        #[inline]
                        #[allow(clippy::identity_op)]
                        fn tlspl_serialized_len(&self) -> usize {
                            let content_len = match self {
                                #(#variant_arms)*
                            };

                            #discr_block + #maybe_extensible + content_len
                        }
                    }
                    #untagged_trait_impl
                    #untagged_discr_fence
                }
            }
        })
    }

    fn impl_serialize(&self) -> darling::Result<TokenStream2> {
        Ok(match self {
            TlsplDeriveTarget::StructNamed(sv) => {
                let span = sv.span();
                let StructNamedTarget {
                    ident,
                    generics,
                    data,
                } = sv.as_ref();
                let generics = augment_generics_ty(generics, ImplTargets::SERIALIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();

                let member_calls = data.fields.iter().filter_map(|f| {
                    if f.attr.skip.is_present() {
                        return None;
                    }

                    let ident = syn::Member::Named(f.ident.clone());
                    Some(if let Some(module_with) = &f.attr.with {
                        quote_spanned! {f.span()=> #module_with::tlspl_serialize_to(&self.#ident, writer)? }
                    } else if is_u8_type(&f.ty) {
                        quote_spanned! {f.span()=> { writer.write_all(&[self.#ident])?; 1 } }
                    } else {
                        quote_spanned! {f.span()=> thalassa::TlsplSerialize::tlspl_serialize_to(&self.#ident, writer)? }
                    })
                });

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSerialize for #ident #ty_generics #where_c {
                        #[inline]
                        fn tlspl_serialize_to<W: thalassa::io::Write>(&self, writer: &mut W) -> thalassa::error::TlsplWriteResult<usize> {
                            let mut written = 0;
                            #(written += #member_calls;)*
                            Ok(written)
                        }
                    }
                }
            }
            TlsplDeriveTarget::StructTuple(sv) => {
                let span = sv.span();
                let StructTupleTarget {
                    ident,
                    generics,
                    data,
                } = sv.as_ref();
                let generics = augment_generics_ty(generics, ImplTargets::SERIALIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();

                let member_calls = data.fields.iter().enumerate().filter_map(|(idx, f)| {
                    if f.attr.skip.is_present() {
                        return None;
                    }

                    let ident = syn::Member::Unnamed(idx.into());
                    Some(if let Some(module_with) = &f.attr.with {
                        quote_spanned! {f.span()=> #module_with::tlspl_serialize_to(&self.#ident, writer)? }
                    } else if is_u8_type(&f.ty) {
                        quote_spanned! {f.span()=> { writer.write_all(&[self.#ident])?; 1 } }
                    } else {
                        quote_spanned! {f.span()=> thalassa::TlsplSerialize::tlspl_serialize_to(&self.#ident, writer)? }
                    })
                });

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSerialize for #ident #ty_generics #where_c {
                        #[inline]
                        fn tlspl_serialize_to<W: thalassa::io::Write>(&self, writer: &mut W) -> thalassa::error::TlsplWriteResult<usize> {
                            let mut written = 0;
                            #(written += #member_calls;)*
                            Ok(written)
                        }
                    }
                }
            }
            TlsplDeriveTarget::StructUnit(sv) => {
                let span = sv.span();
                let StructUnitTarget { ident, generics } = sv.as_ref();
                let generics = augment_generics_ty(generics, ImplTargets::SERIALIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSerialize for #ident #ty_generics #where_c {
                        #[inline]
                        fn tlspl_serialize_to<W: thalassa::io::Write>(&self, writer: &mut W) -> thalassa::error::TlsplWriteResult<usize> {
                            Ok(0)
                        }
                    }
                }
            }
            TlsplDeriveTarget::Enum(sv) => {
                let span = sv.span();
                let discriminants_ts = sv.data.discriminant_consts(sv.as_ref())?;
                let EnumTarget {
                    ident,
                    generics,
                    data,
                    attr,
                    attrs,
                } = sv.as_ref();

                let generics = augment_generics_ty(generics, ImplTargets::SERIALIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();
                let repr = extract_repr_from_attrs(attrs)?;

                let is_extensible = attr.extensible.is_present();
                let is_untagged = attr.untagged.is_present();

                let (extensible_frame, extensible_assert) = if is_extensible {
                    (
                        quote! { written += thalassa::types::write_vlbytes_content_len(content_len, writer)?; },
                        quote! {
                            debug_assert_eq!(field_written, content_len, "Write mismatch");
                        },
                    )
                } else {
                    (quote!(), quote!())
                };

                let variant_arms = data.variants.iter().map(|variant| {
                    let span = variant.span();
                    let variant_is_tuple = matches!(variant, Variant::Tuple(_));
                    let variant_ident = variant.ident();
                    let discriminant = discr_const_ident(variant_ident);
                    let variant_is_catchall = variant.attrs().other.is_present();

                    let (field_mappings, (field_mapping_calls, length_calls)): (Vec<_>, (Vec<_>, Vec<_>)) = variant
                        .members_with_exprs()
                        .filter_map(|member| {
                            if member.attrs.skip.is_present() {
                                return None;
                            }

                            let ident = member.ident;
                            let (method_mapping, length_call) = if let Some(with) = member.attrs.with {
                                (quote_spanned! { member.span=> #with::tlspl_serialize_to(&#ident, writer)? }, quote_spanned! { member.span=> #with::tlspl_serialized_len(&#ident) })
                            } else if is_u8_type(&member.ty) {
                                (quote_spanned! { member.span=> { writer.write_all(&[*#ident])?; 1 } }, quote_spanned! { member.span=> 1 })
                            } else {
                                (quote_spanned! { member.span=> thalassa::TlsplSerialize::tlspl_serialize_to(&#ident, writer)? }, quote_spanned! { member.span=> thalassa::TlsplSize::tlspl_serialized_len(&#ident) })
                            };

                            Some((ident, (method_mapping, length_call)))
                        })
                        .unzip();

                    let variant_match = if variant_is_tuple {
                        quote!(#ident::#variant_ident(#(#field_mappings,)* ..))
                    } else {
                        quote!(#ident::#variant_ident { #(#field_mappings,)* .. })
                    };

                    if variant_is_catchall {
                        if !variant_is_tuple || field_mappings.is_empty() || field_mappings.len() > 2 {
                            return Err(darling::Error::custom("The `other` Variant is malformed, it is expected to be a tuple comprised of 1 or 2 fields: (repr) or (repr, Cow<[u8]>)"));
                        }

                        return Ok(quote_spanned! { span=>
                            #variant_match => {
                                return Ok(0 #(+ #field_mapping_calls)*);
                            },
                        });
                    }

                    let discr_block = if is_untagged {
                        quote!(0)
                    } else {
                        quote!({writer.write_all(&#discriminant.to_be_bytes())?; core::mem::size_of::<#repr>() })
                    };

                    let extensible_len = if is_extensible {
                        quote! { let content_len = 0 #(+ #length_calls)*; }
                    } else {
                        quote!()
                    };

                    Ok(quote_spanned! { span=>
                        #variant_match => {
                            #extensible_len
                            written += #discr_block;
                            #extensible_frame
                            field_written += 0 #(+ #field_mapping_calls)*;
                            #extensible_assert
                            written += field_written;
                        },
                    })
                }).collect::<darling::Result<Vec<_>>>()?;

                let untagged_assert = if attr.untagged.is_present() {
                    quote! {
                        const _: fn() = || {
                            thalassa::util::assert_untagged_enum::<#ident>();
                        };
                    }
                } else {
                    quote!()
                };

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSerialize for #ident #ty_generics #where_c {
                        #[inline]
                        #[allow(clippy::identity_op, non_upper_case_globals)]
                        fn tlspl_serialize_to<W: thalassa::io::Write>(&self, writer: &mut W) -> thalassa::error::TlsplWriteResult<usize> {
                            #discriminants_ts

                            #[allow(unused_imports)]
                            use thalassa::{TlsplSize as _, io::Write as _};

                            let mut written = 0usize;
                            let mut field_written = 0usize;
                            match self {
                                #(#variant_arms)*
                            }

                            Ok(written)
                        }
                    }
                    #untagged_assert
                }
            }
        })
    }

    fn impl_deserialize(&self) -> darling::Result<TokenStream2> {
        Ok(match self {
            TlsplDeriveTarget::StructNamed(sv) => {
                let span = sv.span();
                let StructNamedTarget {
                    ident,
                    generics,
                    data,
                } = sv.as_ref();

                let ty_generics_def = augment_generics_ty(generics, ImplTargets::DESERIALIZE);
                let impl_generics_def = augment_generics_with_lt(&ty_generics_def);
                let (impl_generics, _, _) = impl_generics_def.split_for_impl();
                let (_, ty_generics, where_c) = generics.split_for_impl();

                let struct_members: Vec<MemberWithAttrs> = data
                    .fields
                    .iter()
                    .map(|f| MemberWithAttrs {
                        span: f.span(),
                        ident: f.ident.clone(),
                        ty: f.ty.clone(),
                        attrs: f.attr.clone(),
                    })
                    .collect();

                let select_stores: Vec<Option<TokenStream2>> = struct_members
                    .iter()
                    .enumerate()
                    .map(|(pos, member)| -> darling::Result<_> {
                        Ok(match &member.attrs.select {
                            Some(select_target) => {
                                validate_select_target(member.span, &member.ty)?;
                                let (select_toks, select_target_ident) = resolve_select_target(
                                    select_target,
                                    &struct_members,
                                    pos,
                                    member.span,
                                )?;
                                Some(emit_select_fence_store(
                                    member.span,
                                    &member.ty,
                                    select_toks,
                                    select_target_ident,
                                ))
                            }
                            None => None,
                        })
                    })
                    .collect::<darling::Result<Vec<_>>>()?;

                let (member_list, member_decls): (Vec<_>, Vec<_>) = data
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(pos, f)| {
                        let ident = f.ident.clone();
                        let ty = f.ty.clone();
                        let get_value_tokens = if f.attr.skip.is_present() {
                            quote_spanned! { f.span()=> Default::default() }
                        } else if let Some(with) = &f.attr.with {
                            quote_spanned! { f.span()=> #with::tlspl_deserialize_from(reader)? }
                        } else if is_u8_type(&f.ty) {
                            quote_spanned! { f.span()=> reader.read_byte()? }
                        } else {
                            quote_spanned! { f.span()=> <_>::tlspl_deserialize_from(reader)? }
                        };

                        let select_fence_store = select_stores[pos].clone().unwrap_or_default();

                        (
                            quote!(#ident),
                            quote_spanned! { f.span()=>
                                let #ident: #ty = {
                                    #select_fence_store
                                    #get_value_tokens
                                };
                            },
                        )
                    })
                    .unzip();

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplDeserialize<'tlspl> for #ident #ty_generics #where_c {
                        #[inline]
                        fn tlspl_deserialize_from<R: thalassa::io::Read<'tlspl>>(reader: &mut thalassa::io::DepthAwareReaderGuard<'_, R>) -> thalassa::error::TlsplReadResult<Self> {
                            #[allow(unused_imports)]
                            use thalassa::io::Read as _;
                            #(#member_decls)*
                            Ok(Self {
                                #(#member_list,)*
                            })
                        }
                    }
                }
            }
            TlsplDeriveTarget::StructTuple(sv) => {
                let span = sv.span();
                let StructTupleTarget {
                    ident,
                    generics,
                    data,
                } = sv.as_ref();

                let ty_generics_def = augment_generics_ty(generics, ImplTargets::DESERIALIZE);
                let impl_generics_def = augment_generics_with_lt(&ty_generics_def);
                let (impl_generics, _, _) = impl_generics_def.split_for_impl();
                let (_, ty_generics, where_c) = generics.split_for_impl();

                let member_calls = data.fields.iter().map(|f| {
                    if f.attr.skip.is_present() {
                        quote_spanned! { f.span()=> Default::default() }
                    } else if let Some(with) = &f.attr.with {
                        quote_spanned! { f.span()=> #with::tlspl_deserialize_from(reader)? }
                    } else if is_u8_type(&f.ty) {
                        quote_spanned! {f.span()=> reader.read_byte()? }
                    } else {
                        quote_spanned! { f.span()=> <_>::tlspl_deserialize_from(reader)? }
                    }
                });

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplDeserialize<'tlspl> for #ident #ty_generics #where_c {
                        #[inline]
                        fn tlspl_deserialize_from<R: thalassa::io::Read<'tlspl>>(reader: &mut thalassa::io::DepthAwareReaderGuard<'_, R>) -> thalassa::error::TlsplReadResult<Self> {
                            #[allow(unused_imports)]
                            use thalassa::io::Read as _;
                            Ok(Self(#(#member_calls,)*))
                        }
                    }
                }
            }
            TlsplDeriveTarget::StructUnit(sv) => {
                let span = sv.span();
                let StructUnitTarget { ident, generics } = sv.as_ref();
                let ty_generics_def = augment_generics_ty(generics, ImplTargets::DESERIALIZE);
                let impl_generics_def = augment_generics_with_lt(&ty_generics_def);
                let (impl_generics, _, _) = impl_generics_def.split_for_impl();
                let (_, ty_generics, where_c) = generics.split_for_impl();

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplDeserialize<'tlspl> for #ident #ty_generics #where_c {
                        #[inline]
                        fn tlspl_deserialize_from<R: thalassa::io::Read<'tlspl>>(reader: &mut thalassa::io::DepthAwareReaderGuard<'_, R>) -> thalassa::error::TlsplReadResult<Self> {
                            Ok(Self)
                        }
                    }
                }
            }
            TlsplDeriveTarget::Enum(sv) => {
                let span = sv.span();
                let discriminants_ts = sv.data.discriminant_consts(sv.as_ref())?;
                let EnumTarget {
                    ident,
                    generics,
                    data,
                    attrs,
                    attr,
                } = sv.as_ref();

                let repr = extract_repr_from_attrs(attrs)?;
                let ty_generics_def = augment_generics_ty(generics, ImplTargets::DESERIALIZE);
                let impl_generics_def = augment_generics_with_lt(&ty_generics_def);
                let (impl_generics, _, _) = impl_generics_def.split_for_impl();
                let (_, ty_generics, where_c) = generics.split_for_impl();

                let mut catchall_def = None;

                let extensible_bootstrap = if attr.extensible.is_present() {
                    quote! {
                        let buffer = std::borrow::Cow::<[u8]>::tlspl_deserialize_from(reader)?;
                        let mut buffer = reader.handoff_state(buffer);
                    }
                } else {
                    quote!()
                };

                let extensible_finalize = if attr.extensible.is_present() {
                    quote! {
                        if !buffer.is_eof()? {
                            return Err(thalassa::error::TlsplReadError::TrailingBytes { extra: buffer.remaining().unwrap_or_default() });
                        }
                    }
                } else {
                    quote!()
                };

                let variant_arms = data.variants.iter().filter_map(|variant| {
                    if variant.attrs().other.is_present() {
                        catchall_def.replace((variant.ident().clone(), variant.field_count()));
                        return None;
                    }

                    let span = variant.span();
                    let variant_ident = variant.ident();
                    let variant_is_tuple = matches!(variant, Variant::Tuple(_));
                    let discriminant = discr_const_ident(variant_ident);

                    let read_target = if attr.extensible.is_present() {
                        quote!(&mut buffer.enter()?)
                    } else {
                        quote!(reader)
                    };

                    let variant_members: Vec<MemberWithAttrs> = variant.members_with_exprs().collect();

                    let (field_idents, field_bindings): (Vec<_>, Vec<_>) = variant
                        .members_with_exprs()
                        .enumerate()
                        .map(|(pos, member)| {
                            let method_mapping = if member.attrs.skip.is_present() {
                                quote_spanned! { member.span=> Default::default() }
                            } else if let Some(with) = member.attrs.with {
                                quote_spanned! { member.span=> #with::tlspl_deserialize_from(#read_target)? }
                            }  else if is_u8_type(&member.ty) {
                                quote_spanned! { member.span=> (#read_target).read_byte()? }
                            } else {
                                quote_spanned! { member.span=> <_>::tlspl_deserialize_from(#read_target)? }
                            };

                            let select_fence_store = if let Some(select_target) = &member.attrs.select {
                                let target_res = validate_select_target(member.span, &member.ty).and_then(
                                    |_| {
                                        resolve_select_target(
                                            select_target,
                                            &variant_members,
                                            pos,
                                            member.span,
                                        )
                                    },
                                );

                                match target_res {
                                    Ok((target_toks, target_ident)) => {
                                        emit_select_fence_store(member.span, &member.ty, target_toks, target_ident)
                                    }
                                    Err(err) => err.write_errors(),
                                }
                            } else {
                                quote!()
                            };

                            let ident = member.ident.clone();
                            let ty = member.ty.clone();
                            let binding = quote_spanned! { member.span=>
                                let #ident: #ty = {
                                    #select_fence_store
                                    #method_mapping
                                };
                            };

                            (ident, binding)
                        })
                        .unzip();

                    let variant_splat = if variant_is_tuple {
                        quote! {
                            #ident::#variant_ident(
                                #(#field_idents,)*
                            )
                        }
                    } else {
                        quote! {
                            #ident::#variant_ident {
                                #(#field_idents,)*
                            }
                        }
                    };

                    Some(quote_spanned! { span=>
                        #discriminant => {
                            #(#field_bindings)*
                            let value = #variant_splat;
                            #extensible_finalize
                            value
                        },
                    })
                }).collect::<Vec<_>>();

                let discr_fence_name = discr_fence_name(ident);

                let discr_block = if attr.untagged.is_present() {
                    quote! {
                        let discriminant: #repr = #discr_fence_name.take().ok_or(thalassa::error::TlsplReadError::MissingEnumDiscriminant {
                            enum_type: stringify!(#ident),
                        })?;
                    }
                } else {
                    quote! {
                        let discriminant = <#repr>::from_be_bytes(*reader.read_array()?);
                    }
                };

                let catchall_block = if let Some((catchall_variant, field_count)) = catchall_def {
                    match field_count {
                        1 => quote!(discr => #ident::#catchall_variant(discr)),
                        2 => {
                            quote!(discr => #ident::#catchall_variant(discr, buffer.read_till_eof()?))
                        }
                        _ => {
                            return Err(darling::Error::custom(
                                "Unexpected field count in the catchall enum, need 1 (naked) or 2 (fielded) depending on the enum nature",
                            ));
                        }
                    }
                } else {
                    quote!(_ => return Err(thalassa::error::TlsplReadError::UnknownEnumDiscriminant(discriminant.into())))
                };

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplDeserialize<'tlspl> for #ident #ty_generics #where_c {
                        #[inline]
                        #[allow(clippy::identity_op, non_upper_case_globals)]
                        fn tlspl_deserialize_from<R: thalassa::io::Read<'tlspl>>(reader: &mut thalassa::io::DepthAwareReaderGuard<'_, R>) -> thalassa::error::TlsplReadResult<Self> {
                            #[allow(unused_imports)]
                            use thalassa::io::Read as _;

                            #discriminants_ts
                            #discr_block
                            #extensible_bootstrap
                            Ok(match discriminant {
                                #(#variant_arms)*
                                #catchall_block
                            })
                        }
                    }
                }
            }
        })
    }

    fn impl_all(&self) -> darling::Result<TokenStream2> {
        let mut tokens = self.impl_size()?;
        tokens.extend(self.impl_serialize()?);
        tokens.extend(self.impl_deserialize()?);
        Ok(tokens)
    }
}

#[proc_macro_derive(TlsplSize, attributes(tlspl))]
pub fn derive_size(input: TokenStream) -> TokenStream {
    use darling::FromDeriveInput as _;
    match TlsplDeriveTarget::from_derive_input(&parse_macro_input!(input as DeriveInput)) {
        Ok(target) => match target.impl_size() {
            Ok(tokens) => tokens.into(),
            Err(e) => e.write_errors().into(),
        },
        Err(e) => e.write_errors().into(),
    }
}

#[proc_macro_derive(TlsplSerialize, attributes(tlspl))]
pub fn derive_serialize(input: TokenStream) -> TokenStream {
    use darling::FromDeriveInput as _;
    match TlsplDeriveTarget::from_derive_input(&parse_macro_input!(input as DeriveInput)) {
        Ok(target) => match target.impl_serialize() {
            Ok(tokens) => tokens.into(),
            Err(e) => e.write_errors().into(),
        },
        Err(e) => e.write_errors().into(),
    }
}

#[proc_macro_derive(TlsplDeserialize, attributes(tlspl))]
pub fn derive_deserialize(input: TokenStream) -> TokenStream {
    use darling::FromDeriveInput as _;
    match TlsplDeriveTarget::from_derive_input(&parse_macro_input!(input as DeriveInput)) {
        Ok(target) => match target.impl_deserialize() {
            Ok(tokens) => tokens.into(),
            Err(e) => e.write_errors().into(),
        },
        Err(e) => e.write_errors().into(),
    }
}

#[proc_macro_derive(TlsplAll, attributes(tlspl))]
pub fn derive_all(input: TokenStream) -> TokenStream {
    use darling::FromDeriveInput as _;
    match TlsplDeriveTarget::from_derive_input(&parse_macro_input!(input as DeriveInput)) {
        Ok(target) => match target.impl_all() {
            Ok(tokens) => tokens.into(),
            Err(e) => e.write_errors().into(),
        },
        Err(e) => e.write_errors().into(),
    }
}
