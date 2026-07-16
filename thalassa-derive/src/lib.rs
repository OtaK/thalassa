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
use quote::{quote, quote_spanned};
use syn::{
    Attribute, DeriveInput, Expr, ExprLit, ExprPath, GenericParam, Generics, Ident, Lit,
    parse_macro_input, parse_quote,
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
struct TlsplFieldAttrs {
    /// Custom serialization pointing to a module that has any of those 3 functions:
    /// - tlspl_serialized_len(&T) -> usize
    /// - tlspl_serialize_to(&T, writer) -> WriteResult<usize>
    /// - tlspl_deserialize_from(reader) -> ReadResult<T>
    with: Option<ExprPath>,
    /// Skip this field (requires [`Default`] since serialization has fixed layout)
    skip: Flag,
    #[allow(dead_code)]
    /// Distant variant - WIP
    discriminant_for: Option<Ident>,
}

#[derive(Debug, darling::FromMeta)]
struct TlsplVariantAttrs {
    /// Custom enum discriminant
    discriminant: Option<DiscriminantTarget>,
    /// Marks a "catch-all" or "unknown" variant that needs to
    /// contain a single tuple field equal to the #[repr] of the enum
    ///
    /// Eg: for a `#[repr(u8)]`, this variant needs to contain a `u8` like so:
    /// ```rust,ignore
    /// #[derive(TlsplSize, TlsplDeserialize, TlsplSerialize)]
    /// #[repr(u8)]
    /// enum Thing {
    ///     CaseA = 0,
    ///     CaseB = 1,
    ///     #[tlspl(catchall)]
    ///     Unknown(u8)
    /// }
    /// ```
    #[allow(dead_code)]
    catchall: Flag,
}

fn discr_const_ident(variant_ident: &Ident) -> Ident {
    Ident::new(&format!("__THALASSA_{variant_ident}"), Span::call_site())
}

#[derive(Debug, darling::FromField)]
#[darling(attributes(tlspl))]
struct FieldNamedRaw {
    // TODO: Uncomment + change type to `Ident` when darling 0.24 is out
    // and rename the struct to FieldNamed (+ delete the other one)
    // #[darling(with = darling::util::require_ident)]
    ident: Option<Ident>,
    #[darling(flatten)]
    attr: TlsplFieldAttrs,
}

#[derive(Debug)]
struct FieldNamed {
    ident: Ident,
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
    attr: TlsplVariantAttrs,
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
    attr: TlsplVariantAttrs,
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
    attr: TlsplVariantAttrs,
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
enum Variant {
    Unit(SpannedValue<VariantUnit>),
    Named(SpannedValue<VariantNamed>),
    Tuple(SpannedValue<VariantTuple>),
}

#[derive(Debug)]
struct MemberWithAttrs<'a> {
    span: Span,
    ident: Ident,
    attr_with: Option<&'a ExprPath>,
    attr_skip: bool,
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

    fn attrs(&self) -> &TlsplVariantAttrs {
        match self {
            Variant::Unit(variant_unit) => &variant_unit.attr,
            Variant::Named(variant_named) => &variant_named.attr,
            Variant::Tuple(variant_tuple) => &variant_tuple.attr,
        }
    }

    fn members_with_exprs(&self) -> Box<dyn Iterator<Item = MemberWithAttrs<'_>> + '_> {
        match self {
            Variant::Unit(_) => Box::new(std::iter::empty()),
            Variant::Named(variant_named) => {
                Box::new(variant_named.fields.iter().map(|f| MemberWithAttrs {
                    span: f.span(),
                    ident: f.ident.clone(),
                    attr_with: f.attr.with.as_ref(),
                    attr_skip: f.attr.skip.is_present(),
                }))
            }
            Variant::Tuple(variant_tuple) => {
                Box::new(variant_tuple.fields.iter().enumerate().map(|(idx, f)| {
                    let span = f.span();
                    MemberWithAttrs {
                        ident: Ident::new(&format!("tuple{idx}"), f.span()),
                        span,
                        attr_with: f.attr.with.as_ref(),
                        attr_skip: f.attr.skip.is_present(),
                    }
                }))
            }
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
    #[darling(default = Span::call_site)]
    span: Span,
    ident: Ident,
    generics: Generics,
}

#[derive(Debug, darling::FromDeriveInput)]
struct StructNamedTarget {
    #[darling(default = Span::call_site)]
    span: Span,
    ident: Ident,
    generics: Generics,
    #[darling(with = TryFrom::try_from)]
    data: StructNamedFields,
}

#[derive(Debug, darling::FromDeriveInput)]
struct StructTupleTarget {
    #[darling(default = Span::call_site)]
    span: Span,
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
#[darling(forward_attrs(repr))]
struct EnumTarget {
    #[darling(default = Span::call_site)]
    span: Span,
    ident: Ident,
    generics: Generics,
    attrs: Vec<Attribute>,
    #[darling(with = TryFrom::try_from)]
    data: EnumVariants,
}

#[derive(Debug)]
enum TlsplDeriveTarget {
    StructUnit(StructUnitTarget),
    StructNamed(StructNamedTarget),
    StructTuple(StructTupleTarget),
    Enum(EnumTarget),
}

impl TlsplDeriveTarget {
    #[allow(dead_code)]
    fn span(&self) -> Span {
        match self {
            TlsplDeriveTarget::StructUnit(struct_unit_target) => struct_unit_target.span,
            TlsplDeriveTarget::StructNamed(struct_named_target) => struct_named_target.span,
            TlsplDeriveTarget::StructTuple(struct_tuple_target) => struct_tuple_target.span,
            TlsplDeriveTarget::Enum(enum_target) => enum_target.span,
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
        match &input.data {
            syn::Data::Struct(syn::DataStruct {
                fields: syn::Fields::Named(_),
                ..
            }) => StructNamedTarget::from_derive_input(input).map(Self::StructNamed),
            syn::Data::Struct(syn::DataStruct {
                fields: syn::Fields::Unnamed(_),
                ..
            }) => StructTupleTarget::from_derive_input(input).map(Self::StructTuple),
            syn::Data::Struct(syn::DataStruct {
                fields: syn::Fields::Unit,
                ..
            }) => StructUnitTarget::from_derive_input(input).map(Self::StructUnit),
            syn::Data::Enum(_) => EnumTarget::from_derive_input(input).map(Self::Enum),
            syn::Data::Union(_) => Err(darling::Error::custom(
                "Unions are not supported in the TLSPL data scheme",
            )),
        }
    }
}

impl TlsplDeriveTarget {
    fn impl_size(&self) -> darling::Result<TokenStream2> {
        Ok(match self {
            TlsplDeriveTarget::StructNamed(StructNamedTarget {
                span,
                ident,
                generics,
                data,
            }) => {
                let generics = augment_generics_ty(generics, ImplTargets::SIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();

                let member_calls = data.fields.iter().filter_map(|f| {
                    if f.attr.skip.is_present() {
                        return None;
                    }

                    let ident = f.ident.clone();
                    Some(if let Some(module_with) = &f.attr.with {
                        quote_spanned! {f.span()=> #module_with::tlspl_serialized_len(&self.#ident) }
                    } else {
                        quote_spanned! {f.span()=> self.#ident.tlspl_serialized_len() }
                    })
                });

                quote_spanned! { *span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSize for #ident #ty_generics #where_c {
                        #[inline]
                        fn tlspl_serialized_len(&self) -> usize {
                            0 #(+ #member_calls)*
                        }
                    }
                }
            }
            TlsplDeriveTarget::StructTuple(StructTupleTarget {
                span,
                ident,
                generics,
                data,
            }) => {
                let generics = augment_generics_ty(generics, ImplTargets::SIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();

                let member_calls = data.fields.iter().enumerate().filter_map(|(idx, f)| {
                    if f.attr.skip.is_present() {
                        return None;
                    }

                    let ident = syn::Member::Unnamed(idx.into());
                    Some(if let Some(module_with) = &f.attr.with {
                        quote_spanned! { f.span()=> #module_with::tlspl_serialized_len(&self.#ident) }
                    } else {
                        quote_spanned! { f.span()=> self.#ident.tlspl_serialized_len() }
                    })
                });

                quote_spanned! { *span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSize for #ident #ty_generics #where_c {
                        #[inline]
                        fn tlspl_serialized_len(&self) -> usize {
                            0 #(+ #member_calls)*
                        }
                    }
                }
            }
            TlsplDeriveTarget::StructUnit(StructUnitTarget {
                span,
                ident,
                generics,
            }) => {
                let generics = augment_generics_ty(generics, ImplTargets::SIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();

                quote_spanned! { *span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSize for #ident #ty_generics #where_c {
                        #[inline]
                        fn tlspl_serialized_len(&self) -> usize { 0 }
                    }
                }
            }
            Self::Enum(EnumTarget {
                span,
                ident,
                generics,
                attrs,
                data,
            }) => {
                let repr = extract_repr_from_attrs(attrs)?;
                let generics = augment_generics_ty(generics, ImplTargets::SIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();
                // let (_, ty_generics, where_c) = generics.split_for_impl();
                // let (impl_generics, ty_generics, where_c) = generics.split_for_impl();
                //
                let field_arms = data.variants.iter().map(|variant| {
                    let span = variant.span();
                    let variant_ident = variant.ident();
                    let variant_is_tuple = matches!(variant, Variant::Tuple(_));

                    let (field_mappings, field_mapping_calls): (Vec<_>, Vec<_>) = variant.members_with_exprs().filter_map(|member| {
                        if member.attr_skip {
                            return None;
                        }

                        let ident = member.ident;
                        let method_mapping = if let Some(with) = member.attr_with {
                            quote_spanned! { member.span=> #with::tlspl_serialized_len(&#ident) }
                        } else {
                            quote_spanned! { member.span=> #ident.tlspl_serialized_len() }
                        };

                        Some((ident, method_mapping))
                    }).unzip();

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

                quote_spanned! { *span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSize for #ident #ty_generics #where_c {
                        #[inline]
                        #[allow(clippy::identity_op)]
                        fn tlspl_serialized_len(&self) -> usize {
                            core::mem::size_of::<#repr>() + match self {
                                #(#field_arms)*
                            }
                        }
                    }
                }
            }
        })
    }

    fn impl_serialize(&self) -> darling::Result<TokenStream2> {
        Ok(match self {
            TlsplDeriveTarget::StructNamed(StructNamedTarget {
                span,
                ident,
                generics,
                data,
            }) => {
                let generics = augment_generics_ty(generics, ImplTargets::SERIALIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();

                let member_calls = data.fields.iter().filter_map(|f| {
                    if f.attr.skip.is_present() {
                        return None;
                    }

                    let ident = f.ident.clone();
                    Some(if let Some(module_with) = &f.attr.with {
                        quote_spanned! {f.span()=> #module_with::tlspl_serialize_to(&self.#ident, writer)? }
                    } else {
                        quote_spanned! {f.span()=> self.#ident.tlspl_serialize_to(writer)? }
                    })
                });

                quote_spanned! { *span=>
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
            TlsplDeriveTarget::StructTuple(StructTupleTarget {
                span,
                ident,
                generics,
                data,
            }) => {
                let generics = augment_generics_ty(generics, ImplTargets::SERIALIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();

                let member_calls = data.fields.iter().enumerate().filter_map(|(idx, f)| {
                    if f.attr.skip.is_present() {
                        return None;
                    }

                    let ident = syn::Member::Unnamed(idx.into());
                    Some(if let Some(module_with) = &f.attr.with {
                        quote_spanned! {f.span()=> #module_with::tlspl_serialize_to(&self.#ident, writer)? }
                    } else {
                        quote_spanned! {f.span()=> self.#ident.tlspl_serialize_to(writer)? }
                    })
                });

                quote_spanned! { *span=>
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
            TlsplDeriveTarget::StructUnit(StructUnitTarget {
                span,
                ident,
                generics,
            }) => {
                let generics = augment_generics_ty(generics, ImplTargets::SERIALIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();

                quote_spanned! { *span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSerialize for #ident #ty_generics #where_c {
                        #[inline]
                        fn tlspl_serialize_to<W: thalassa::io::Write>(&self, writer: &mut W) -> thalassa::error::TlsplWriteResult<usize> {
                            Ok(0)
                        }
                    }
                }
            }
            TlsplDeriveTarget::Enum(target) => {
                let discriminants_ts = target.data.discriminant_consts(target)?;
                let EnumTarget {
                    span,
                    ident,
                    generics,
                    data,
                    ..
                } = target;
                let generics = augment_generics_ty(generics, ImplTargets::SERIALIZE);
                let (impl_generics, ty_generics, where_c) = generics.split_for_impl();

                let field_arms = data.variants.iter().map(|variant| {
                    let span = variant.span();
                    let variant_is_tuple = matches!(variant, Variant::Tuple(_));
                    let variant_ident = variant.ident();
                    let discriminant = discr_const_ident(variant_ident);
                    let (field_mappings, field_mapping_calls): (Vec<_>, Vec<_>) = variant
                        .members_with_exprs()
                        .filter_map(|member| {
                            if member.attr_skip {
                                return None;
                            }

                            let ident = member.ident;
                            let method_mapping = if let Some(with) = member.attr_with {
                                quote_spanned! { member.span=> #with::tlspl_serialize_to(&#ident, writer)? }
                            } else {
                                quote_spanned! { member.span=> #ident.tlspl_serialize_to(writer)? }
                            };

                            Some((ident, method_mapping))
                        })
                        .unzip();

                    if variant_is_tuple {
                        quote_spanned! { span=>
                            #ident::#variant_ident(#(#field_mappings,)* ..) => {
                                writer.write(&#discriminant.to_be_bytes())?
                                #(+ #field_mapping_calls)*
                            },
                        }
                    } else {
                        quote_spanned! { span=>
                            #ident::#variant_ident { #(#field_mappings,)* .. } => {
                                writer.write(&#discriminant.to_be_bytes())?
                                #(+ #field_mapping_calls)*
                            },
                        }
                    }
                });

                quote_spanned! { *span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplSerialize for #ident #ty_generics #where_c {
                        #[inline]
                        #[allow(clippy::identity_op)]
                        fn tlspl_serialize_to<W: thalassa::io::Write>(&self, writer: &mut W) -> thalassa::error::TlsplWriteResult<usize> {
                            #discriminants_ts
                            Ok(match self {
                                #(#field_arms)*
                            })
                        }
                    }
                }
            }
        })
    }

    fn impl_deserialize(&self) -> darling::Result<TokenStream2> {
        Ok(match self {
            TlsplDeriveTarget::StructNamed(StructNamedTarget {
                span,
                ident,
                generics,
                data,
            }) => {
                let ty_generics_def = augment_generics_ty(generics, ImplTargets::DESERIALIZE);
                let impl_generics_def = augment_generics_with_lt(&ty_generics_def);
                let (impl_generics, _, _) = impl_generics_def.split_for_impl();
                let (_, ty_generics, where_c) = generics.split_for_impl();

                let member_calls = data.fields.iter().map(|f| {
                    let ident = f.ident.clone();
                    if f.attr.skip.is_present() {
                        quote_spanned! { f.span()=> #ident: Default::default() }
                    } else if let Some(with) = &f.attr.with {
                        quote_spanned! { f.span()=> #ident: #with::tlspl_deserialize_from(reader)? }
                    } else {
                        quote_spanned! { f.span()=> #ident: <_>::tlspl_deserialize_from(reader)? }
                    }
                });

                quote_spanned! { *span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplDeserialize<'tlspl> for #ident #ty_generics #where_c {
                        #[inline]
                        fn tlspl_deserialize_from<R: thalassa::io::Read<'tlspl>>(reader: &mut R) -> thalassa::error::TlsplReadResult<Self> {
                            Ok(Self {
                                #(#member_calls,)*
                            })
                        }
                    }
                }
            }
            TlsplDeriveTarget::StructTuple(StructTupleTarget {
                span,
                ident,
                generics,
                data,
            }) => {
                let ty_generics_def = augment_generics_ty(generics, ImplTargets::DESERIALIZE);
                let impl_generics_def = augment_generics_with_lt(&ty_generics_def);
                let (impl_generics, _, _) = impl_generics_def.split_for_impl();
                let (_, ty_generics, where_c) = generics.split_for_impl();

                let member_calls = data.fields.iter().map(|f| {
                    if f.attr.skip.is_present() {
                        quote_spanned! { f.span()=> Default::default() }
                    } else if let Some(with) = &f.attr.with {
                        quote_spanned! { f.span()=> #with::tlspl_deserialize_from(reader)? }
                    } else {
                        quote_spanned! { f.span()=> <_>::tlspl_deserialize_from(reader)? }
                    }
                });

                quote_spanned! { *span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplDeserialize<'tlspl> for #ident #ty_generics #where_c {
                        #[inline]
                        fn tlspl_deserialize_from<R: thalassa::io::Read<'tlspl>>(reader: &mut R) -> thalassa::error::TlsplReadResult<Self> {
                            Ok(Self(#(#member_calls,)*))
                        }
                    }
                }
            }
            TlsplDeriveTarget::StructUnit(StructUnitTarget {
                span,
                ident,
                generics,
            }) => {
                let ty_generics_def = augment_generics_ty(generics, ImplTargets::DESERIALIZE);
                let impl_generics_def = augment_generics_with_lt(&ty_generics_def);
                let (impl_generics, _, _) = impl_generics_def.split_for_impl();
                let (_, ty_generics, where_c) = generics.split_for_impl();

                quote_spanned! { *span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplDeserialize<'tlspl> for #ident #ty_generics #where_c {
                        #[inline]
                        fn tlspl_deserialize_from<R: thalassa::io::Read<'tlspl>>(reader: &mut R) -> thalassa::error::TlsplReadResult<Self> {
                            Ok(Self)
                        }
                    }
                }
            }
            TlsplDeriveTarget::Enum(target) => {
                let discriminants_ts = target.data.discriminant_consts(target)?;
                let EnumTarget {
                    span,
                    ident,
                    generics,
                    data,
                    attrs,
                } = target;
                let repr = extract_repr_from_attrs(attrs)?;
                let ty_generics_def = augment_generics_ty(generics, ImplTargets::DESERIALIZE);
                let impl_generics_def = augment_generics_with_lt(&ty_generics_def);
                let (impl_generics, _, _) = impl_generics_def.split_for_impl();
                let (_, ty_generics, where_c) = generics.split_for_impl();

                let variant_arms = data.variants.iter().map(|variant| {
                    let span = variant.span();
                    let variant_ident = variant.ident();
                    let variant_is_tuple = matches!(variant, Variant::Tuple(_));
                    let discriminant = discr_const_ident(variant_ident);
                    let (field_mappings, field_mapping_calls): (Vec<_>, Vec<_>) = variant
                        .members_with_exprs()
                        .map(|member| {
                            let method_mapping = if member.attr_skip {
                                quote_spanned! { member.span=> Default::default() }
                            } else if let Some(with) = member.attr_with {
                                quote_spanned! { member.span=> #with::tlspl_deserialize_from(reader)? }
                            } else {
                                quote_spanned! { member.span=> <_>::tlspl_deserialize_from(reader)? }
                            };

                            (member.ident, method_mapping)
                        })
                        .unzip();
                    if variant_is_tuple {
                        quote_spanned! { span=>
                            #discriminant => Ok(#ident::#variant_ident(
                                #(#field_mapping_calls,)*
                            )),
                        }
                    } else {
                        quote_spanned! { span=>
                            #discriminant => Ok(#ident::#variant_ident {
                                #(#field_mappings: #field_mapping_calls,)*
                            }),
                        }
                    }
                });

                quote_spanned! { *span=>
                    #[automatically_derived]
                    impl #impl_generics thalassa::TlsplDeserialize<'tlspl> for #ident #ty_generics #where_c {
                        #[inline]
                        #[allow(clippy::identity_op)]
                        fn tlspl_deserialize_from<R: thalassa::io::Read<'tlspl>>(reader: &mut R) -> thalassa::error::TlsplReadResult<Self> {
                            #discriminants_ts
                            let discriminant = <#repr>::from_be_bytes(*reader.read_array()?);
                            match discriminant {
                                #(#variant_arms)*
                                _ => Err(thalassa::error::TlsplReadError::UnknownEnumDiscriminant(discriminant.into()))
                            }
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
