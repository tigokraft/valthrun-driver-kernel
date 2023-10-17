use core::{
    marker::PhantomData, cell::SyncUnsafeCell,
};

use crate::ll;

/// Low level system import which does not use any heap allocation.
/// It will return None if the import has failed.
///
/// Attention:
/// Due to the restrictions and expected performance, while resolving the import
/// race conditions can occurr and the import gets resolved multiple times.
pub struct LLSystemExport<'a, T> {
    function: &'a str,
    value: SyncUnsafeCell<*const ()>,
    _dummy: PhantomData<T>,
}

unsafe impl<T> Sync for LLSystemExport<'_, T> {}
unsafe impl<T> Send for LLSystemExport<'_, T> {}

impl<'a, T> LLSystemExport<'a, T> {
    pub const fn new(function: &'a str) -> Self {
        Self {
            function,
            value: SyncUnsafeCell::new(core::ptr::null_mut()),
            _dummy: PhantomData {},
        }
    }

    pub fn resolve(&self) -> Option<T> {
        let value = unsafe { &mut *self.value.get() };
        if value.is_null() {
            *value = ll::lookup_system_export(self.function)? as *const ();
        }

        Some(unsafe { core::mem::transmute_copy(value) })
    }
}
