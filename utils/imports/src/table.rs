use core::{
    cell::SyncUnsafeCell,
    fmt::Debug,
    mem::MaybeUninit,
    sync::atomic::{
        AtomicU8,
        Ordering,
    },
};

use obfstr::obfstr;

use crate::ImportResult;

/// Initializer for the import table.
/// Will only be called once
pub type ImportTableInitializer<T> = dyn (Fn() -> ImportResult<T>) + Send + Sync;

/// An import which can be dynamically resolved at runtime
pub trait DynamicImport<T>: Debug {
    fn resolve(self) -> ImportResult<T>;
}

const TABLE_STATE_UNINITIALIZED: u8 = 0;
const TABLE_STATE_RESOLVING: u8 = 1;
const TABLE_STATE_RESOLVED: u8 = 2;
pub struct DynamicImportTable<T: 'static> {
    init: &'static ImportTableInitializer<T>,
    table: SyncUnsafeCell<MaybeUninit<T>>,
    table_state: AtomicU8,
}

impl<T> DynamicImportTable<T> {
    pub const fn new(init: &'static ImportTableInitializer<T>) -> Self {
        Self {
            init,
            table: SyncUnsafeCell::new(MaybeUninit::uninit()),
            table_state: AtomicU8::new(TABLE_STATE_UNINITIALIZED),
        }
    }

    pub fn get(&self) -> Option<&T> {
        if self.table_state.load(Ordering::Relaxed) == TABLE_STATE_RESOLVED {
            Some(unsafe { (*self.table.get()).assume_init_ref() })
        } else {
            None
        }
    }

    pub fn resolve(&self) -> ImportResult<&T> {
        loop {
            match self.table_state.compare_exchange(
                TABLE_STATE_UNINITIALIZED,
                TABLE_STATE_RESOLVING,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    /* initialize the variable */
                    match (*self.init)() {
                        Ok(result) => {
                            unsafe { (*self.table.get()).write(result) };
                            self.table_state
                                .store(TABLE_STATE_RESOLVED, Ordering::Relaxed);
                        }
                        Err(err) => {
                            self.table_state
                                .store(TABLE_STATE_UNINITIALIZED, Ordering::Relaxed);
                            return Err(err);
                        }
                    }
                }
                Err(TABLE_STATE_RESOLVING) => { /* table is getting initialized, keep looping */ }
                Err(TABLE_STATE_RESOLVED) => {
                    return unsafe { Ok((*self.table.get()).assume_init_ref()) };
                }
                _ => unreachable!(),
            }
        }
    }

    pub fn unwrap(&self) -> &T {
        match self.resolve() {
            Ok(table) => table,
            Err(error) => panic!("{}: {:#}", obfstr!("Failed to load import table"), error),
        }
    }
}

impl<T> Drop for DynamicImportTable<T> {
    fn drop(&mut self) {
        match self.table_state.load(Ordering::Relaxed) {
            TABLE_STATE_RESOLVED => {
                unsafe {
                    /* drop the value */
                    (*self.table.get()).assume_init_drop();
                }
            }
            TABLE_STATE_RESOLVING => panic!("the import table has been dropped while resolving"),
            TABLE_STATE_UNINITIALIZED => { /* nothing to do */ }
            _ => unreachable!(),
        }
    }
}
