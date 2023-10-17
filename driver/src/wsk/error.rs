use embedded_io::{
    Error,
    ErrorKind,
};
use thiserror::Error;
use utils_imports::ImportError;
use winapi::shared::ntdef::NTSTATUS;

pub type WskResult<T> = core::result::Result<T, WskError>;

#[derive(Error, Debug)]
pub enum WskError {
    #[error("failed to resolve imports: {0:#}")]
    ImportError(ImportError),

    #[error("failed to register: {0:X}")]
    Register(NTSTATUS),

    #[error("failed to capture wsk provider: {0:X}")]
    CaptureProvider(NTSTATUS),

    #[error("insufficient resources for {0}")]
    OutOfMemory(&'static str),

    #[error("invalid buffer")]
    InvalidBuffer,

    #[error("timeout")]
    Timeout,

    #[error("query result is null")]
    QueryResultNull,

    #[error("{0:X}")]
    SocketCreation(NTSTATUS),

    #[error("{0:X}")]
    OperationFailed(NTSTATUS),
}

impl Error for WskError {
    fn kind(&self) -> embedded_io::ErrorKind {
        match self {
            Self::OutOfMemory(_) => ErrorKind::OutOfMemory,
            Self::Timeout => ErrorKind::TimedOut,
            _ => ErrorKind::Other,
        }
    }
}
