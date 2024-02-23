use alloc::sync::Arc;

use utils_imports::{
    dynamic_import_table,
    provider::SystemExport,
};
use winapi::km::wdm::{
    self,
    PDEFERRED_ROUTINE,
};

type KeInitializeDpc = unsafe extern "C" fn(
    Dpc: *mut wdm::KDPC,
    DeferredRoutine: PDEFERRED_ROUTINE,
    DeferredContext: *const (),
);
type KeInsertQueueDpc = unsafe extern "C" fn(
    Dpc: *const wdm::KDPC,
    SystemArgument1: *const (),
    SystemArgument2: *const (),
);

dynamic_import_table! {
    /// These imports should not fail!
    imports DYNAMIC_IMPORTS {
        //pub DbgPrintEx: DbgPrintEx = SystemExport::new(obfstr!("DbgPrintEx")),
        pub KeInitializeDpc: KeInitializeDpc = SystemExport::new(obfstr!("KeInitializeDpc")),
        pub KeInsertQueueDpc: KeInsertQueueDpc = SystemExport::new(obfstr!("KeInsertQueueDpc")),
    }
}

#[derive(Clone)]
pub struct Dpc {
    inner: Arc<wdm::KDPC>,
}

unsafe impl Sync for Dpc {}
unsafe impl Send for Dpc {}

impl Dpc {
    pub fn new(deferred_routine: PDEFERRED_ROUTINE, deferred_context: *const ()) -> Self {
        let imports = DYNAMIC_IMPORTS.unwrap();
        let kdpc = unsafe {
            let dpc = Arc::new_zeroed().assume_init();
            (imports.KeInitializeDpc)(
                core::mem::transmute(&*dpc),
                deferred_routine,
                deferred_context,
            );
            dpc
        };

        Self { inner: kdpc }
    }

    pub fn enqueue(&self, system_argument1: *const (), system_argument2: *const ()) {
        if let Some(imports) = DYNAMIC_IMPORTS.get() {
            unsafe {
                (imports.KeInsertQueueDpc)(&*self.inner, system_argument1, system_argument2);
            }
        } else {
            /*
             * This should not happen as we initialized this object already.
             * Just in case, as this is used while logging just silently drop this error.
             */
        }
    }
}

impl Drop for Dpc {
    fn drop(&mut self) {
        /* FIXME: Cancel the DPC! */
    }
}
