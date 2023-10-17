use alloc::string::String;
use core::convert::Infallible;

use embedded_io::WriteFmtError;
use thiserror::Error;

use crate::wsk::WskError;

#[derive(Error, Debug)]
pub enum HttpError {
    #[error("dns lookup failed: {0}")]
    DnsLookupFailure(WskError),

    #[error("dns lookup yielded no results")]
    DnsNoResults,

    #[error("unexpected EOF")]
    EOF,

    #[error("io: {0}")]
    WskTransportError(#[from] WskError),

    #[error("tls: {0:?}")]
    TlsTransportError(embedded_tls::TlsError),

    #[error("connect failed: {0}")]
    ConnectError(anyhow::Error),

    #[error("response headers too long")]
    ResponseHeadersTooLong,

    #[error("uncomplete response headers")]
    ResponseHeadersUncomplete,

    #[error("invalid response header {0}")]
    ResponseHeaderInvalid(String),

    #[error("response headers invalid: {0}")]
    ResponseHeadersInvalid(httparse::Error),

    #[error("fmt error")]
    WriteFmtError(WriteFmtError<Infallible>),
}

impl From<embedded_tls::TlsError> for HttpError {
    fn from(value: embedded_tls::TlsError) -> Self {
        HttpError::TlsTransportError(value)
    }
}
