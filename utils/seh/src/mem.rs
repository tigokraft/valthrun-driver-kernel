use kdef::_PEB;
use utils_imports::{
    dynamic_import_table,
    provider::SystemExport,
    ImportResult,
};
use winapi::km::{
    ndis::PMDL,
    wdm::KPROCESSOR_MODE,
};

use crate::wrapper;

dynamic_import_table! {
    imports MEM_IMPORTS {
        pub ProbeForRead: u64 = SystemExport::new(obfstr!("ProbeForRead")),
        pub ProbeForWrite: u64 = SystemExport::new(obfstr!("ProbeForWrite")),
        pub memmove: u64 = SystemExport::new(obfstr!("memmove")),
        pub MmProbeAndLockProcessPages: u64 = SystemExport::new(obfstr!("MmProbeAndLockProcessPages")),

        pub PsGetCurrentProcess: unsafe extern "C" fn() -> *mut _PEB = SystemExport::new(obfstr!("PsGetCurrentProcess")),
    }
}

pub fn init() -> ImportResult<()> {
    MEM_IMPORTS.resolve().map(|_| ())
}

pub fn probe_read(target: u64, length: usize, align: usize) -> bool {
    let target_fn = MEM_IMPORTS.unwrap().ProbeForRead;
    unsafe { wrapper::seh_invoke(target_fn, target, length as u64, align as u64, 0) }
}

pub fn probe_write(target: u64, length: usize, align: usize) -> bool {
    let target_fn = MEM_IMPORTS.unwrap().ProbeForWrite;
    unsafe { wrapper::seh_invoke(target_fn, target, length as u64, align as u64, 0) }
}

pub fn probe_and_lock_pages(mdl: PMDL, access_mode: KPROCESSOR_MODE, operation: u32) -> bool {
    /*
     * We must use MmProbeAndLockProcessPages instead of MmProbeAndLockPages as
     * MmProbeAndLockPages writes to the shaddow stack, which we do not support.
     * MmProbeAndLockProcessPages is identical when Process == PsGetCurrentProcess() therefore we're good here.
     */
    let imports = MEM_IMPORTS.unwrap();
    let target = imports.MmProbeAndLockProcessPages;
    let current_process = unsafe { (imports.PsGetCurrentProcess)() };

    unsafe {
        wrapper::seh_invoke(
            target,
            mdl as u64,
            current_process as u64,
            access_mode as u64,
            operation as u64,
        )
    }
}

/// Copy memory from source into target.
/// Returns false on failure.
pub fn safe_copy(target: &mut [u8], source: u64) -> bool {
    let target_fn = MEM_IMPORTS.unwrap().memmove;
    unsafe {
        wrapper::seh_invoke(
            target_fn,
            target.as_mut_ptr() as u64,
            source,
            target.len() as u64,
            0,
        )
    }
}
