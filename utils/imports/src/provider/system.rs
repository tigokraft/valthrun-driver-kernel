use core::fmt::Debug;

use alloc::string::ToString;
use obfstr::obfstr;

use crate::{DynamicImport, ll, ImportResult, ImportError};

#[derive(Debug)]
pub struct SystemExport<'a> {
    function: &'a str,
}

impl<'a> SystemExport<'a> {
    pub fn new(function: &'a str) -> Self {
        Self { function }
    }
}

impl<'a, T> DynamicImport<T> for SystemExport<'a> {
    fn resolve(self) -> ImportResult<T> {
        ll::lookup_system_export(self.function)
            .map(|value| unsafe { core::mem::transmute_copy(&value) })
            .ok_or_else(|| ImportError::SymbolUnknown { module: obfstr!("system").to_string(), symbol: self.function.to_string() })
    }
}
