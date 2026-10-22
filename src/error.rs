/// Payload of `Custom`, boxed so the error enums stay as small as possible.
#[derive(Debug, thiserror::Error)]
#[error("crate({krate}) Error: {message}")]
pub struct CustomError {
    pub krate: &'static str,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TlsplWriteError {
    #[error(transparent)]
    Parsio(#[from] parsio::WriteError),
    #[error("The length of the vector exceeds the admissible limits")]
    LengthOverflow,
    #[error(transparent)]
    Custom(Box<CustomError>),
}

impl TlsplWriteError {
    pub fn custom(calling_crate: &'static str, message: impl Into<String>) -> Self {
        Self::Custom(Box::new(CustomError {
            krate: calling_crate,
            message: message.into(),
        }))
    }
}

pub type TlsplWriteResult<T> = Result<T, TlsplWriteError>;

struct FmtTrailing(usize);

impl std::fmt::Display for FmtTrailing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == usize::MAX {
            f.write_str("trailing bytes after the message (count unknown)")
        } else {
            write!(
                f,
                "{} trailing byte(s) after the message; the input was not fully consumed",
                self.0
            )
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TlsplReadError {
    #[error(transparent)]
    Parsio(#[from] parsio::ReadError),
    #[error(transparent)]
    IntegerOverflow(#[from] std::num::TryFromIntError),
    #[error(transparent)]
    Utf8StringError(#[from] simdutf8::basic::Utf8Error),
    #[error("The length of the vector exceeds the admissible limits")]
    LengthOverflow,
    #[error("The length byte in this vlbytes instance would cause an overflow")]
    VlBytesLengthOverflow,
    #[error("Could not deserialize enum with discriminant {0:X}")]
    UnknownEnumDiscriminant(u64),
    #[error(
        "Set semantics violation: A duplicate entry has been detected, and deserializing such a value would result in inconsistencies"
    )]
    DuplicateSetValue,
    #[error(
        "Map semantics violation: A duplicate key has been detected, and deserializing such a key would result in inconsistencies"
    )]
    DuplicateMapKey,
    #[error("Illegal `bool` value; must be 0 or 1")]
    IllegalBool,
    #[error("The `optional<T>` is prefixed by a value different from 0 or 1")]
    IllegalOptionPrefix,
    #[error("No discriminant was found for untagged enum `{enum_type}`")]
    MissingEnumDiscriminant { enum_type: &'static str },
    /// `extra` equals `usize::MAX` when the trailing bytes count is unknown
    #[error("{}", FmtTrailing(*extra))]
    TrailingBytes { extra: usize },
    #[error("ContentLength mismatch, expected {expected} but got {got}")]
    ContentLengthMismatch { expected: usize, got: usize },
    #[error("The element <T> of this collection is zero-sized. This is not allowed in TLSPL")]
    ZeroSizedCollectionElement,
    #[error(transparent)]
    Custom(Box<CustomError>),
}

impl TlsplReadError {
    pub fn custom(calling_crate: &'static str, message: impl Into<String>) -> Self {
        Self::Custom(Box::new(CustomError {
            krate: calling_crate,
            message: message.into(),
        }))
    }
}

pub type TlsplReadResult<T> = Result<T, TlsplReadError>;

#[derive(Debug, thiserror::Error)]
pub enum TlsplError {
    #[error("Read error: {0}")]
    Read(#[from] TlsplReadError),
    #[error("Write error: {0}")]
    Write(#[from] TlsplWriteError),
}

pub type TlsplResult<T> = Result<T, TlsplError>;
