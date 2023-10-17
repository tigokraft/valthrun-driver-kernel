use alloc::boxed::Box;
use core::fmt::Debug;

use obfstr::obfstr;

pub mod ll;

mod system_ll;
pub use system_ll::*;

mod system;
pub use system::*;

mod module;
pub use module::*;

pub trait DynamicImport<T>: Debug {
    fn resolve(self) -> anyhow::Result<T>;
}

pub type ImportTableInitializer<T> = dyn (Fn() -> anyhow::Result<T>) + Send + Sync;
pub struct DynamicImportTable<T: 'static> {
    init: &'static ImportTableInitializer<T>,
    table: once_cell::race::OnceBox<T>,
}

/* All resolved imports must be Send & Sync (else they would not be exported) */
unsafe impl<T> Sync for DynamicImportTable<T> {}
unsafe impl<T> Send for DynamicImportTable<T> {}

impl<T> DynamicImportTable<T> {
    pub const fn new(init: &'static ImportTableInitializer<T>) -> Self {
        Self {
            init,
            table: once_cell::race::OnceBox::new(),
        }
    }

    pub fn get(&self) -> Option<&T> {
        self.table.get()
    }

    pub fn resolve(&self) -> anyhow::Result<&T> {
        self.table.get_or_try_init(|| Ok(Box::new((*self.init)()?)))
    }

    pub fn unwrap(&self) -> &T {
        match self.resolve() {
            Ok(table) => table,
            Err(error) => panic!("{}: {:#}", obfstr!("Failed to load import table"), error),
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
