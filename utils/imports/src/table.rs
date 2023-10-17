use alloc::boxed::Box;
use core::fmt::Debug;

use obfstr::obfstr;

use crate::ImportResult;

pub trait DynamicImport<T>: Debug {
    fn resolve(self) -> ImportResult<T>;
}

pub type ImportTableInitializer<T> = dyn (Fn() -> ImportResult<T>) + Send + Sync;
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

    pub fn resolve(&self) -> ImportResult<&T> {
        self.table.get_or_try_init(|| Ok(Box::new((*self.init)()?)))
    }

    pub fn unwrap(&self) -> &T {
        match self.resolve() {
            Ok(table) => table,
            Err(error) => panic!("{}: {:#}", obfstr!("Failed to load import table"), error),
        }
    }
}
