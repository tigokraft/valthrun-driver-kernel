use alloc::string::String;
use thiserror::Error;

pub type ImportResult<T> = Result<T, ImportError>;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("module {module} not found")]
    ModuleUnknown{ module: String },
    
    #[error("symbol {symbol} in {module} not found")]
    SymbolUnknown{ module: String, symbol: String },

    /// A generic import error has occurred
    #[error("{reason}")]
    Generic{ reason: String },
}