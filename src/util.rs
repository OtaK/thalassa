/// Marker trait to denote an enum as having the attribute `#[tlspl(untagged)]` set.
/// This is used internally by the derive and shouldn't be implemented by yourself,
/// and as such is an `unsafe` trait
///
/// The reason it's public is that it needs to be exposed for our const assertions
/// to work properly
///
/// ## Safety
///
/// N/A, it's just a marker trait for static assertions
pub unsafe trait TlsplUntaggedEnum: crate::TlsplSize {}

pub mod reexports {
    pub use static_assertions;
}

#[macro_export]
/// Asserts that the target struct implements [`TlsplUntaggedEnum`]
macro_rules! assert_untagged {
    ($target:ty) => {
        $crate::util::reexports::static_assertions::assert_impl_all!(
            $target: $crate::util::TlsplUntaggedEnum
        );
    };
}
