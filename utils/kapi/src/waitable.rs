use core::time::Duration;

use alloc::boxed::Box;
use utils_imports::{dynamic_import_table, provider::SystemExport};
use winapi::{shared::{ntdef::{PVOID, NTSTATUS, LIST_ENTRY, WaitAll, WaitAny}, ntstatus::{STATUS_SUCCESS, STATUS_WAIT_0, STATUS_WAIT_63}}, km::wdm::KPROCESSOR_MODE};

pub type KeWaitForSingleObject = unsafe extern "C" fn(
    Object: PVOID,
    WaitReason: i32,
    WaitMode: KPROCESSOR_MODE,
    Alertable: bool,
    Timeout: *const i64,
) -> NTSTATUS;

pub type KeWaitForMultipleObjects = unsafe extern "C" fn(
    Count: u32,
    Object: PVOID,
    WaitType: u32,
    WaitReason: i32,
    WaitMode: KPROCESSOR_MODE,
    Alertable: bool,
    Timeout: *const i64,
    WaitBlockArray: *const _KWAIT_BLOCK
) -> NTSTATUS;

dynamic_import_table! {
    pub imports WAIT_IMPORTS {
        pub KeWaitForSingleObject: KeWaitForSingleObject = SystemExport::new(obfstr!("KeWaitForSingleObject")),
        pub KeWaitForMultipleObjects: KeWaitForMultipleObjects = SystemExport::new(obfstr!("KeWaitForMultipleObjects")),
    }
}

#[allow(non_snake_case, non_camel_case_types)]
pub struct _KWAIT_BLOCK {
    WaitListEntry: LIST_ENTRY,
    WaitType: u8,
    BlockState: u8, /* volatile */
    WaitKey: u16,
    SpareLong: i32,
    _Thread_NotificationQueue_Dpc: PVOID,
    Object: PVOID,
    SparePtr: PVOID,
}

/// Object supports `KeWaitForSingleObject` and `KeWaitForMultipleObjects`
pub trait Waitable {
    fn waitable(&self) -> &dyn Waitable;
    
    fn wait_object(&self) -> PVOID;

    fn wait_for(&self, reason: i32, mode: KPROCESSOR_MODE, alertable: bool, timeout: Option<Duration>) -> bool {
        let imports = WAIT_IMPORTS.unwrap();
        let timeout = timeout.map(|value| (value.as_nanos() / 100) as i64 * -1);

        let status = unsafe {
            (imports.KeWaitForSingleObject)(
                self.wait_object(),
                reason,
                mode,
                alertable,
                if let Some(timeout) = &timeout { timeout as *const _ } else { core::ptr::null() }
            )
        };

        status == STATUS_SUCCESS
    }
}

pub trait MultipleWait<T> {
    fn wait_all<'a>(
        &self,
        reason: i32, 
        mode: KPROCESSOR_MODE,
        alertable: bool, 
        timeout: Option<Duration>
    ) -> bool;
    
    fn wait_any<'a>(
        &self,
        reason: i32, 
        mode: KPROCESSOR_MODE,
        alertable: bool, 
        timeout: Option<Duration>
    ) -> Option<usize>;
}

impl<const N: usize> MultipleWait<[ &dyn Waitable; N ]> for [ &dyn Waitable; N ] {
    fn wait_all<'a>(
        &self,
        reason: i32, 
        mode: KPROCESSOR_MODE,
        alertable: bool, 
        timeout: Option<Duration>
    ) -> bool {
        let objects = self.map(|value| value.wait_object());
        let block_array = if N > 3 {
            Some(unsafe { Box::<[_KWAIT_BLOCK; N]>::new_zeroed().assume_init() })
        } else {
            None
        };

        let imports = WAIT_IMPORTS.unwrap();
        let timeout = timeout.map(|value| (value.as_nanos() / 100) as i64 * -1);

        let status = unsafe {
            (imports.KeWaitForMultipleObjects)(
                N as u32, objects.as_ptr() as PVOID,
                WaitAll,
                reason,
                mode,
                alertable,
                if let Some(timeout) = &timeout { timeout as *const _ } else { core::ptr::null() },
                if let Some(array) = &block_array { array.as_ptr() } else { core::ptr::null_mut() }
            )
        };

        status == STATUS_SUCCESS
    }

    fn wait_any<'a>(
        &self,
        reason: i32, 
        mode: KPROCESSOR_MODE,
        alertable: bool, 
        timeout: Option<Duration>
    ) -> Option<usize> {
        let objects = self.map(|value| value.wait_object());
        let block_array = if N > 3 {
            Some(unsafe { Box::<[_KWAIT_BLOCK; N]>::new_zeroed().assume_init() })
        } else {
            None
        };

        let imports = WAIT_IMPORTS.unwrap();
        let timeout = timeout.map(|value| (value.as_nanos() / 100) as i64 * -1);

        let status = unsafe {
            (imports.KeWaitForMultipleObjects)(
                N as u32, objects.as_ptr() as PVOID,
                WaitAny,
                reason,
                mode,
                alertable,
                if let Some(timeout) = &timeout { timeout as *const _ } else { core::ptr::null() },
                if let Some(array) = &block_array { array.as_ptr() } else { core::ptr::null_mut() }
            )
        };

        match status {
            STATUS_WAIT_0..=STATUS_WAIT_63 => Some((status - STATUS_WAIT_0) as usize),
            _ => None
        }
    }
}