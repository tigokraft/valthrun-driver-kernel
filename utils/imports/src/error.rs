use thiserror::Error;

pub type ImportResult<T> = Result<T, DynamicImportError>;

#[derive(Debug, Error)]
pub enum DynamicImportError {
    #[error("the target provider has not been initialized")]
    ProviderNotInitialized,

    #[error("the target module can not be found")]
    ModuleUnknown,

    #[error("the target symbol can not be found")]
    SymbolUnknown,

    /// A generic import error has occurred
    #[error("{reason}")]
    Generic { reason: &'static str },
}
