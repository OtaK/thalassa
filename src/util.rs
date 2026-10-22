/// Marker trait to denote an enum as having the attribute `#[tlspl(untagged)]` set.
/// This is used internally by the derive and shouldn't be implemented by yourself,
/// and as such is an `unsafe` trait
///
/// The reason it's public is that it needs to be exposed for our in-derive
/// const assertions to work properly
///
/// ## Safety
///
/// N/A, it's just a marker trait for static assertions
#[diagnostic::on_unimplemented(
    message = "a `#[tlspl(select = ...)]` field must be an enum with `#[tlspl(untagged)]`",
    label = "`{Self}` is not a `#[tlspl(untagged)]` enum",
    note = "declare the field as that enum type itself"
)]
#[doc(hidden)]
pub unsafe trait TlsplUntaggedEnum: crate::TlsplSize {
    type Discriminant: Copy;
    fn __thalassa_discr_fence()
    -> &'static std::thread::LocalKey<std::cell::Cell<Option<Self::Discriminant>>>;
}

/// Asserts that the target struct implements [`TlsplUntaggedEnum`]
#[inline(always)]
pub const fn assert_untagged_enum<T: TlsplUntaggedEnum>() {}
