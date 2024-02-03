use utils_imports::{
    utils,
    DynamicImport,
    DynamicImportError,
    ImportResult,
};

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
            .map_err(|_err| {
                DynamicImportError::Generic {
                    reason: "failed to iterate modules ",
                }
            })?
            .ok_or_else(|| DynamicImportError::ModuleUnknown)?;

        utils::resolve_symbol_from_pimage(net_module.base_address as u64, self.symbol)
            .map(|value| unsafe { core::mem::transmute_copy(&value) })
            .ok_or_else(|| DynamicImportError::SymbolUnknown)
    }
}
