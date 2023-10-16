use alloc::{
    boxed::Box,
    ffi::CString,
    format,
    string::{
        String,
        ToString,
    },
};
use core::fmt::Debug;

use anyhow::{
    anyhow,
    Context,
};
use obfstr::obfstr;
use winapi::shared::ntdef::{
    PVOID,
    UNICODE_STRING,
};

use crate::{
    kapi::{
        KModule,
        UnicodeStringEx,
    },
    kdef::MmGetSystemRoutineAddress,
};

pub trait DynamicImport<T>: Debug {
    fn resolve(self) -> anyhow::Result<T>;
}

#[derive(Debug)]
pub struct SystemExport {
    function: &'static [u16],
}

impl SystemExport {
    pub fn new(function: &'static [u16]) -> Self {
        Self { function }
    }
}

impl<T> DynamicImport<T> for SystemExport {
    fn resolve(self) -> anyhow::Result<T> {
        let uname = UNICODE_STRING::from_bytes(self.function);
        unsafe {
            let address = MmGetSystemRoutineAddress(&uname);
            if address.is_null() {
                anyhow::bail!(
                    "{} {}",
                    obfstr!("failed to resolve system function"),
                    String::from_utf16_lossy(self.function)
                )
            } else {
                Ok(core::mem::transmute_copy(&address))
            }
        }
    }
}

type RtlFindExportedRoutineByName = unsafe extern "C" fn(PVOID, *const i8) -> PVOID;
dynamic_import_table! {
    imports SYS_IMPORTS {
        pub RtlFindExportedRoutineByName: RtlFindExportedRoutineByName = SystemExport::new(obfstr::wide!("RtlFindExportedRoutineByName")),
    }
}

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
        let sys_imports = SYS_IMPORTS
            .resolve()
            .with_context(|| obfstr!("failed to resolve system imports").to_string())?;

        let net_module = KModule::find_by_name(self.module)?
            .with_context(|| format!("{} {}", obfstr!("failed to find module"), self.module))?;

        let cfunction = CString::new(self.function)
            .map_err(|_| anyhow!("{}", obfstr!("function name contains null character")))?;
        let function = unsafe {
            (sys_imports.RtlFindExportedRoutineByName)(
                net_module.base_address as PVOID,
                cfunction.as_ptr(),
            )
            .cast::<()>()
        };

        if function.is_null() {
            anyhow::bail!(
                "{} {} in module {} ({:X})",
                obfstr!("failed to find"),
                self.function,
                self.module,
                net_module.base_address
            )
        }

        Ok(unsafe { core::mem::transmute_copy(&function) })
    }
}

pub type ImportTableInitializer<T> = dyn (Fn() -> anyhow::Result<T>) + Send + Sync;
pub struct DynamicImportTable<T: 'static> {
    init: &'static ImportTableInitializer<T>,
    table: once_cell::race::OnceBox<T>,
}

impl<T> DynamicImportTable<T> {
    pub const fn new(init: &'static ImportTableInitializer<T>) -> Self {
        Self {
            init,
            table: once_cell::race::OnceBox::new(),
        }
    }

    pub fn resolve(&self) -> anyhow::Result<&T> {
        self.table.get_or_try_init(|| Ok(Box::new((*self.init)()?)))
    }

    pub fn unwrap(&self) -> &T {
        match self.resolve() {
            Ok(table) => table,
            Err(error) => panic!("{}: {:#}", obfstr!("Failed to load import table"), error)
        }
    }
}

#[macro_export]
macro_rules! dynamic_import_table {
    (
        $(#[$struct_meta:meta])*
        $visibility:vis imports $name:ident {
            $(pub $var_name:ident: $var_type:ty = $var_init:expr,)*
        }
    ) => {
        paste::paste! {
            #[allow(non_camel_case_types, non_snake_case, unused)]
            $(#[$struct_meta])*
            $visibility struct [<_ $name>] {
                $(pub $var_name: $var_type,)*
            }

            impl [<_ $name>] {
                pub fn resolve() -> anyhow::Result<Self> {
                    use $crate::util::imports::DynamicImport;
                    use anyhow::Context;
                    use obfstr::obfstr;

                    Ok(Self {
                        $(
                            $var_name: {
                                ($var_init).resolve().with_context(|| ::alloc::format!("{} {:?}", obfstr!("Failed to resolve dynamic import "), $var_init))?
                            }
                            ,
                        )*
                    })
                }
            }

            $visibility static $name: $crate::util::imports::DynamicImportTable<[<_ $name>]> = $crate::util::imports::DynamicImportTable::new(&[<_ $name>]::resolve);
        }
    };
}

pub use dynamic_import_table;
