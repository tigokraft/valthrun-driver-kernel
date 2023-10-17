use alloc::{format, string::ToString};
use utils_imports::{DynamicImport, ImportResult, ImportError, ll};

use crate::KModule;


#[derive(Debug)]
pub struct ModuleExport {
    module: &'static str,
    symbol: &'static str,
}

impl ModuleExport {
    pub fn new(module: &'static str, symbol: &'static str) -> Self {
        Self { module, symbol }
    }
}

impl<T> DynamicImport<T> for ModuleExport {
    fn resolve(self) -> ImportResult<T> {
        let net_module = KModule::find_by_name(self.module)
            .map_err(|err| ImportError::Generic { reason: format!("{:#}", err) })?
            .ok_or_else(|| ImportError::ModuleUnknown { module: self.module.to_string() })?;

        ll::lookup_export(net_module.base_address as u64, self.symbol)
            .map(|value| unsafe { core::mem::transmute_copy(&value) })
            .ok_or_else(|| ImportError::SymbolUnknown { module: self.module.to_string(), symbol: self.symbol.to_string() })
    }
}
