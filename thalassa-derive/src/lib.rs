#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

// Note: A lot of this is similar to tls_codec's derives, in an effort to ease migration between the derives/attrs

use darling::{
    FromField, FromVariant as _,
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

use crate::util::from_field_named_raw_to_actual;

#[derive(Debug, Clone)]
enum DiscriminantTarget {
    Lit(usize),
    Path(ExprPath),
}

impl darling::FromMeta for DiscriminantTarget {
    fn from_value(lit: &Lit) -> darling::Result<Self> {
        Ok(match lit {
            Lit::Str(lit_str) => Self::Path(lit_str.parse()?),
            Lit::Byte(lit_byte) => Self::Lit(lit_byte.value() as _),
            Lit::Int(lit_int) => Self::Lit(lit_int.base10_parse()?),
            _ => {
                return Err(darling::Error::custom(
                    "expected path (`path::to::CONST::or::Enum::Variant`), integer or byte literal (`b'y'` / `69` (nice))",
                ).with_span(lit));
            }
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
    ///
    /// ### Example
    ///
    /// ```rust,ignore
    /// #[derive(TlsplAll)]
    /// #[repr(u8)]
    /// pub enum FieldDiscriminant {
    ///     Variant1 = 0x01,
    ///     Variant2 = 0x02,
    /// }
    ///
    /// #[derive(TlsplAll)]
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
    ///     pub another_field: Cow<'a, [u8]>,
    ///     #[tlspl(select = field_type)]
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
    /// ```rust,ignore
    /// #[derive(TlsplSize, TlsplDeserialize, TlsplSerialize)]
    /// #[repr(u8)]
    /// enum Thing {
    ///     CaseA = 0,
    ///     CaseB = 1,
    ///     #[tlspl(other)]
    ///     Unknown(u8)
    /// }
    /// ```
    other: Flag,
}

fn discr_const_ident(variant_ident: &Ident) -> Ident {
    quote::format_ident!("__THALASSA_{}", variant_ident)
}

#[derive(Debug, darling::FromField)]
#[darling(attributes(tlspl))]
struct FieldNamedRaw {
    // TODO: Uncomment + change type to `Ident` when darling 0.24 is out
    // and rename the struct to FieldNamed (+ delete the other one)
    // #[darling(with = darling::util::require_ident)]
    ident: Option<Ident>,
    ty: Type,
    #[darling(flatten)]
    attr: TlsplFieldAttrs,
}

#[derive(Debug)]
struct FieldNamed {
    ident: Ident,
    ty: Type,
    attr: TlsplFieldAttrs,
}

impl FromField for FieldNamed {
    fn from_field(field: &syn::Field) -> darling::Result<Self> {
        FieldNamedRaw::from_field(field).map(from_field_named_raw_to_actual)
    }
}

// TODO: Remove this once darling 0.24 is out with the fixes on adding #[darling(with)] on Ident
mod util {
    use darling::util::SpannedValue;

    use crate::{FieldNamed, FieldNamedRaw};

    pub fn from_field_named_raw_to_actual(fnr: FieldNamedRaw) -> FieldNamed {
        FieldNamed {
            ident: fnr
                .ident
                .expect("Implementation error, named fields ALWAYS have an identifier"),
            ty: fnr.ty,
            attr: fnr.attr,
        }
    }

    pub fn map_spanned_raw_to_field(srf: SpannedValue<FieldNamedRaw>) -> SpannedValue<FieldNamed> {
        let span = srf.span();
        SpannedValue::new(from_field_named_raw_to_actual(srf.into_inner()), span)
    }
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
            .filter_map(|field| {
                errors.handle(
                    // TODO: Simplify to just `SpannedValue::<FieldNamed>::from_field` when darling 0.24 is out
                    SpannedValue::<FieldNamedRaw>::from_field(field)
                        .map(util::map_spanned_raw_to_field),
                )
            })
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
    ViaAttr(DiscriminantTarget),
    Explicit(usize),
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
        let explicit_discr = if let Some(Expr::Lit(ExprLit { lit, .. })) = &self.discriminant {
            let discr = match lit {
                Lit::Byte(lit_byte) => lit_byte.value() as usize,
                Lit::Int(lit_int) => lit_int.base10_parse::<usize>()?,
                _ => return Err(darling::Error::custom("The native discriminant cannot be parsed, it should be either an integer value or a byte literal").with_span(span)),
            };
            Some(discr)
        } else {
            None
        };

        if let Some(target) = &self.attr.discriminant {
            // Small sanity check
            if let (Some(native_discr), DiscriminantTarget::Lit(custom_discr)) =
                (&explicit_discr, target)
                && native_discr != custom_discr
            {
                return Err(darling::Error::custom("The explicit and custom discriminants are mismatched, this would result in weird errors").with_span(span));
            }

            return Ok(VariantDiscriminant::ViaAttr(target.clone()));
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
        let explicit_discr = if let Some(Expr::Lit(ExprLit { lit, .. })) = &self.discriminant {
            let discr = match lit {
                Lit::Byte(lit_byte) => lit_byte.value() as usize,
                Lit::Int(lit_int) => lit_int.base10_parse::<usize>()?,
                _ => return Err(darling::Error::custom("The native discriminant cannot be parsed, it should be either an integer value or a byte literal").with_span(span)),
            };
            Some(discr)
        } else {
            None
        };

        if let Some(target) = &self.attr.discriminant {
            // Small sanity check
            if let (Some(native_discr), DiscriminantTarget::Lit(custom_discr)) =
                (&explicit_discr, target)
                && native_discr != custom_discr
            {
                return Err(darling::Error::custom("The explicit and custom discriminants are mismatched, this would result in weird errors").with_span(span));
            }

            return Ok(VariantDiscriminant::ViaAttr(target.clone()));
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
        let explicit_discr = if let Some(Expr::Lit(ExprLit { lit, .. })) = &self.discriminant {
            let discr = match lit {
                Lit::Byte(lit_byte) => lit_byte.value() as usize,
                Lit::Int(lit_int) => lit_int.base10_parse::<usize>()?,
                _ => return Err(darling::Error::custom("The native discriminant cannot be parsed, it should be either an integer value or a byte literal").with_span(span)),
            };
            Some(discr)
        } else {
            None
        };

        if let Some(target) = &self.attr.discriminant {
            // Small sanity check
            if let (Some(native_discr), DiscriminantTarget::Lit(custom_discr)) =
                (&explicit_discr, target)
                && native_discr != custom_discr
            {
                return Err(darling::Error::custom("The explicit and custom discriminants are mismatched, this would result in weird errors").with_span(span));
            }

            return Ok(VariantDiscriminant::ViaAttr(target.clone()));
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
            let mut implicit_discr = 0usize;
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
                    VariantDiscriminant::ViaAttr(discriminant_target) => {
                        match discriminant_target {
                            DiscriminantTarget::Lit(value) => {
                                if path_used {
                                    return Err(path_used_err(&span));
                                }

                                implicit_discr = value;
                                quote_spanned! { span=>
                                    #[allow(non_upper_case_globals)]
                                    const #const_id: #repr = {
                                        if #value < #repr::MIN as usize || #value > #repr::MAX as usize {
                                            panic!("enum repr overflow");
                                        }

                                        #value as #repr
                                    };
                                }
                            }
                            DiscriminantTarget::Path(expr_path) => {
                                path_used = true;
                                quote_spanned! { span=>
                                    #[allow(clippy::unnecessary_cast, non_upper_case_globals)]
                                    const #const_id: #repr = {
                                        let expr_us = #expr_path as usize;
                                        if expr_us < #repr::MIN as usize || expr_us > #repr::MAX as usize {
                                            panic!("enum repr overflow");
                                        }

                                        #expr_path as #repr
                                    };
                                }
                            }
                        }
                    }
                    VariantDiscriminant::Explicit(value) => {
                        if path_used {
                            return Err(path_used_err(&span));
                        }

                        implicit_discr = value;

                        quote_spanned! { span=>
                            #[allow(non_upper_case_globals)]
                            const #const_id: #repr = #implicit_discr as #repr;
                        }
                    }
                    VariantDiscriminant::Implicit => {
                        if path_used {
                            return Err(path_used_err(&span));
                        }

                        quote_spanned! { span=>
                            #[allow(non_upper_case_globals)]
                            const #const_id: #repr = {
                                if #implicit_discr > #repr::MAX as usize {
                                    panic!("enum repr overflow: too many variants");
                                }
                                #implicit_discr as #repr
                            };
                        }
                    }
                };

                implicit_discr += 1;
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
        .and_then(|repr| if repr == "usize" || repr == "isize" {
            Err(darling::Error::custom("`usize` and `isize` reprs are forbidden, as their byte representation depends on pointer size"))
        } else {
            Ok(repr)
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
                                Some(DiscriminantTarget::Path(_))
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
                    } else if f.ty == parse_quote!(u8) {
                        quote_spanned! {f.span()=> 1 }
                    } else {
                        quote_spanned! {f.span()=> self.#ident.tlspl_serialized_len() }
                    })
                });

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSize for #ident #ty_generics #where_c {
                        #[inline]
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
                    } else if f.ty == parse_quote!(u8) {
                        quote_spanned! {f.span()=> 1 }
                    } else {
                        quote_spanned! { f.span()=> self.#ident.tlspl_serialized_len() }
                    })
                });

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSize for #ident #ty_generics #where_c {
                        #[inline]
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
                        } else if member.ty == parse_quote!(u8) {
                            quote_spanned! { member.span=> 1 }
                        } else {
                            quote_spanned! { member.span=> #ident.tlspl_serialized_len() }
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
                            unsafe impl #impl_generics thalassa::util::TlsplUntaggedEnum for #ident #ty_generics #where_c {}
                        },
                        quote! {

                            std::thread_local! {
                                #[allow(non_upper_case_globals)]
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
                    } else if f.ty == parse_quote!(u8) {
                        quote_spanned! {f.span()=> writer.write(&[self.#ident])? }
                    } else {
                        quote_spanned! {f.span()=> self.#ident.tlspl_serialize_to(writer)? }
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
                    } else if f.ty == parse_quote!(u8) {
                        quote_spanned! {f.span()=> writer.write(&[self.#ident])? }
                    } else {
                        quote_spanned! {f.span()=> self.#ident.tlspl_serialize_to(writer)? }
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

                let (extensible_bootstrap, write_target, extensible_finalize) = if is_extensible {
                    (
                        quote! {
                            let mut buffer = Vec::<u8>::with_capacity(self.tlspl_serialized_len() - core::mem::size_of::<#repr>() - 1);
                        },
                        quote!(&mut buffer),
                        quote! {{
                            debug_assert_eq!(field_written, buffer.len(), "Write mismatch");
                            buffer.tlspl_serialize_to(writer)?
                        }},
                    )
                } else {
                    (quote!(), quote!(writer), quote!(field_written))
                };

                let variant_arms = data.variants.iter().map(|variant| {
                    let span = variant.span();
                    let variant_is_tuple = matches!(variant, Variant::Tuple(_));
                    let variant_ident = variant.ident();
                    let discriminant = discr_const_ident(variant_ident);
                    let variant_is_catchall = variant.attrs().other.is_present();

                    let actual_write_target = if variant_is_catchall {
                        quote!(writer)
                    } else {
                        quote!(#write_target)
                    };

                    let (field_mappings, field_mapping_calls): (Vec<_>, Vec<_>) = variant
                        .members_with_exprs()
                        .filter_map(|member| {
                            if member.attrs.skip.is_present() {
                                return None;
                            }

                            let ident = member.ident;
                            let method_mapping = if let Some(with) = member.attrs.with {
                                quote_spanned! { member.span=> #with::tlspl_serialize_to(&#ident, #actual_write_target)? }
                            } else if member.ty == parse_quote!(u8) {
                                quote_spanned! { member.span=> #actual_write_target.write(&[*#ident])? }
                            } else {
                                quote_spanned! { member.span=> #ident.tlspl_serialize_to(#actual_write_target)? }
                            };

                            Some((ident, method_mapping))
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

                    let discr_block = if attr.untagged.is_present() {
                        quote!(0)
                    } else {
                        quote!(writer.write(&#discriminant.to_be_bytes())?)
                    };

                    Ok(quote_spanned! { span=>
                        #variant_match => {
                            #extensible_bootstrap
                            written += #discr_block;
                            field_written += 0 #(+ #field_mapping_calls)*;
                            written += #extensible_finalize;
                        },
                    })
                }).collect::<darling::Result<Vec<_>>>()?;

                let untagged_assert = if attr.untagged.is_present() {
                    quote!(thalassa::assert_untagged!(#ident);)
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
                            use thalassa::TlsplSize as _;

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

                let (member_list, member_decls): (Vec<_>, Vec<_>) = data
                    .fields
                    .iter()
                    .map(|f| {
                        let ident = f.ident.clone();
                        let ty = f.ty.clone();
                        let get_value_tokens = if f.attr.skip.is_present() {
                            quote_spanned! { f.span()=> Default::default() }
                        } else if let Some(with) = &f.attr.with {
                            quote_spanned! { f.span()=> #with::tlspl_deserialize_from(reader)? }
                        } else if f.ty == parse_quote!(u8) {
                            quote_spanned! {f.span()=> reader.read_byte()? }
                        } else {
                            quote_spanned! { f.span()=> <_>::tlspl_deserialize_from(reader)? }
                        };

                        let select_fence_store = if let Some(select_target) = &f.attr.select {
                            let syn::Type::Path(tp) = &f.ty else {
                                panic!("Type is not an actual path");
                            };
                            let ty_ident = tp.path.segments.last().expect("Type cannot be found");
                            let fence_ident = discr_fence_name(&ty_ident.ident);
                            let mut fence_tp = tp.path.clone();
                            let fence_tp_ty = fence_tp.segments.last_mut().unwrap();
                            fence_tp_ty.arguments = Default::default();
                            fence_tp_ty.ident = fence_ident;

                            quote! {
                                #fence_tp.replace(Some(#select_target as _));
                            }
                        } else {
                            quote!()
                        };

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
                        fn tlspl_deserialize_from<R: thalassa::io::Read<'tlspl>>(reader: &mut R) -> thalassa::error::TlsplReadResult<Self> {
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
                    } else if f.ty == parse_quote!(u8) {
                        quote_spanned! {f.span()=> reader.read_byte()? }
                    } else {
                        quote_spanned! { f.span()=> <_>::tlspl_deserialize_from(reader)? }
                    }
                });

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplDeserialize<'tlspl> for #ident #ty_generics #where_c {
                        #[inline]
                        fn tlspl_deserialize_from<R: thalassa::io::Read<'tlspl>>(reader: &mut R) -> thalassa::error::TlsplReadResult<Self> {
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
                        fn tlspl_deserialize_from<R: thalassa::io::Read<'tlspl>>(reader: &mut R) -> thalassa::error::TlsplReadResult<Self> {
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

                let mut catchall_ident = None;

                let extensible_bootstrap = if attr.extensible.is_present() {
                    quote! {
                        let mut buffer = std::borrow::Cow::<[u8]>::tlspl_deserialize_from(reader)?;
                    }
                } else {
                    quote!()
                };

                let variant_arms = data.variants.iter().filter_map(|variant| {
                    if variant.attrs().other.is_present() {
                        catchall_ident.replace(variant.ident().clone());
                        return None;
                    }

                    let span = variant.span();
                    let variant_ident = variant.ident();
                    let variant_is_tuple = matches!(variant, Variant::Tuple(_));
                    let discriminant = discr_const_ident(variant_ident);

                    let read_target = if attr.extensible.is_present() {
                        quote!(&mut buffer)
                    } else {
                        quote!(reader)
                    };

                    let (field_mappings, field_mapping_calls): (Vec<_>, Vec<_>) = variant
                        .members_with_exprs()
                        .map(|member| {
                            let method_mapping = if member.attrs.skip.is_present() {
                                quote_spanned! { member.span=> Default::default() }
                            } else if let Some(with) = member.attrs.with {
                                quote_spanned! { member.span=> #with::tlspl_deserialize_from(#read_target)? }
                            }  else if member.ty == parse_quote!(u8) {
                                quote_spanned! { member.span=> #read_target.read_byte()? }
                            } else {
                                quote_spanned! { member.span=> <_>::tlspl_deserialize_from(#read_target)? }
                            };

                            (member.ident, method_mapping)
                        })
                        .unzip();

                    let variant_splat = if variant_is_tuple {
                        quote! {
                            #ident::#variant_ident(
                                #(#field_mapping_calls,)*
                            )
                        }
                    } else {
                        quote! {
                            #ident::#variant_ident {
                                #(#field_mappings: #field_mapping_calls,)*
                            }
                        }
                    };

                    Some(quote_spanned! { span=>
                        #discriminant => #variant_splat,
                    })
                }).collect::<Vec<_>>();

                let discr_fence_name = discr_fence_name(ident);

                let discr_block = if attr.untagged.is_present() {
                    quote! {
                        let discriminant: #repr = #discr_fence_name.take().expect("There should be a value in the discriminant fence");
                    }
                } else {
                    quote! {
                        let discriminant = <#repr>::from_be_bytes(*reader.read_array()?);
                    }
                };

                let catchall_block = if let Some(catchall_variant) = catchall_ident {
                    quote!(discr => #ident::#catchall_variant(discr, buffer))
                } else {
                    quote!(_ => return Err(thalassa::error::TlsplReadError::UnknownEnumDiscriminant(discriminant.into())))
                };

                quote_spanned! { span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplDeserialize<'tlspl> for #ident #ty_generics #where_c {
                        #[inline]
                        #[allow(clippy::identity_op, non_upper_case_globals)]
                        fn tlspl_deserialize_from<R: thalassa::io::Read<'tlspl>>(reader: &mut R) -> thalassa::error::TlsplReadResult<Self> {
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
