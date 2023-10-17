use core::fmt::Debug;

use anyhow::anyhow;
use obfstr::obfstr;

use super::{
    ll,
    DynamicImport,
};

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
    fn resolve(self) -> anyhow::Result<T> {
        ll::lookup_system_export(self.function)
            .map(|value| unsafe { core::mem::transmute_copy(&value) })
            .ok_or_else(|| {
                anyhow!(
                    "{} {}",
                    obfstr!("failed to resolve system function"),
                    self.function
                )
            })
    }
}
