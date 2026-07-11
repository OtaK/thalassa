#[derive(Debug, thiserror::Error)]
pub enum TlsplWriteError {
    #[error(transparent)]
    Parsio(#[from] parsio::WriteError),
}
pub type TlsplWriteResult<T> = Result<T, TlsplWriteError>;

#[derive(Debug, thiserror::Error)]
pub enum TlsplReadError {
    #[error(transparent)]
    Parsio(#[from] parsio::ReadError),
    #[error(transparent)]
    IntegerOverflow(#[from] std::num::TryFromIntError),
    #[error(transparent)]
    Utf8StringError(#[from] simdutf8::basic::Utf8Error),
    #[error("The length byte in this vlbytes instance would cause an overflow")]
    VlBytesLengthOverflow,
    #[error("Could not deserialize enum with discriminant {0:X}")]
    UnknownEnumDiscriminant(u64),
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
