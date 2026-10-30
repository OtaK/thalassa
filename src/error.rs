/// Payload of `Custom`, boxed so the error enums stay as small as possible.
#[derive(Debug, thiserror::Error)]
#[error("crate({krate}) Error: {message}")]
pub struct CustomError {
    pub krate: &'static str,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TlsplWriteError {
    #[error(transparent)]
    Parsio(#[from] parsio::WriteError),
    #[error("The length of the vector exceeds the admissible limits")]
    LengthOverflow,
    #[error(transparent)]
    Custom(Box<CustomError>),
}

impl From<std::io::Error> for TlsplWriteError {
    fn from(e: std::io::Error) -> Self {
        Self::Parsio(e.into())
    }
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
#[non_exhaustive]
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
    #[error(
        "The length prefix {value} is not in its shortest form, which would make this encoding non-unique"
    )]
    NonCanonicalLength { value: usize },
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
    /// Zero-byte frames can never make progress in a stream.
    #[error("A frame consumed no bytes; a zero-sized item type cannot be framed in a stream")]
    ZeroSizedFrame,
    /// The input ended inside a length-prefixed field; `missing` more bytes are needed at least.
    /// Only returned by readers that know their length (in-memory slices).
    #[error("Input truncated: at least {missing} more byte(s) needed")]
    Incomplete { missing: usize },
    #[error("Frame exceeds the maximum frame length of {max} bytes")]
    FrameTooLarge { max: usize },
    #[error("A value overruns the length-prefixed field enclosing it")]
    NestedTruncation,
    #[error(transparent)]
    Custom(Box<CustomError>),
}

impl From<std::io::Error> for TlsplReadError {
    fn from(e: std::io::Error) -> Self {
        Self::Parsio(e.into())
    }
}

impl TlsplReadError {
    pub fn custom(calling_crate: &'static str, message: impl Into<String>) -> Self {
        Self::Custom(Box::new(CustomError {
            krate: calling_crate,
            message: message.into(),
        }))
    }

    /// `true` when the input simply ended early: more bytes could still make it parse.
    pub fn is_truncation(&self) -> bool {
        matches!(
            self,
            Self::Incomplete { .. }
                | Self::Parsio(parsio::ReadError::IoError(
                    std::io::ErrorKind::UnexpectedEof
                ))
        )
    }

    /// Turns a truncation into [`Self::NestedTruncation`]: used where the input is an already
    /// complete sub-buffer, so running short means malformed, not "wait for more bytes".
    #[doc(hidden)]
    pub fn seal_truncation(self) -> Self {
        if self.is_truncation() {
            Self::NestedTruncation
        } else {
            self
        }
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
