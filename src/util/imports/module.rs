use alloc::format;

use anyhow::{
    anyhow,
    Context,
};
use obfstr::obfstr;

use super::{
    ll,
    DynamicImport,
};
use crate::kapi::KModule;

#[derive(Debug)]
pub struct ModuleExport {
    module: &'static str,
    function: &'static str,
}

impl ModuleExport {
    pub fn new(module: &'static str, function: &'static str) -> Self {
        Self { module, function }
    }
}

impl<T> DynamicImport<T> for ModuleExport {
    fn resolve(self) -> anyhow::Result<T> {
        let net_module = KModule::find_by_name(self.module)?
            .with_context(|| format!("{} {}", obfstr!("failed to find module"), self.module))?;

        ll::lookup_export(net_module.base_address as u64, self.function)
            .map(|value| unsafe { core::mem::transmute_copy(&value) })
            .ok_or_else(|| {
                anyhow!(
                    "{} {} in module {} ({:X})",
                    obfstr!("failed to find"),
                    self.function,
                    self.module,
                    net_module.base_address
                )
            })
    }
}
