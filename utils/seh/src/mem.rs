#![allow(static_mut_refs)]

use kdef::_PEB;
use lazy_link::lazy_link;
use winapi::km::wdm::KPROCESSOR_MODE;

use crate::wrapper;

#[lazy_link(resolver = "kapi_kmodule::resolve_import")]
extern "C" {
    #[allow(non_snake_case)]
    fn PsGetCurrentProcess() -> *mut _PEB;
}

struct FunctionTable {
    probe_for_read: u64,
    probe_for_write: u64,
    memmove: u64,
    mm_probe_and_lock_process_pages: u64,
}

impl FunctionTable {
    pub fn resolve() -> Self {
        Self {
            probe_for_read: kapi_kmodule::resolve_import(None, "ProbeForRead").as_ptr() as u64,
            probe_for_write: kapi_kmodule::resolve_import(None, "ProbeForWrite").as_ptr() as u64,
            memmove: kapi_kmodule::resolve_import(None, "memmove").as_ptr() as u64,
            mm_probe_and_lock_process_pages: kapi_kmodule::resolve_import(
                None,
                "MmProbeAndLockProcessPages",
            )
            .as_ptr() as u64,
        }
    }
}

static mut FUNCTION_TABLE: Option<FunctionTable> = None;

pub fn initialize() {
    unsafe {
        FUNCTION_TABLE = Some(FunctionTable::resolve());
    }
}

pub fn probe_read(target: u64, length: usize, align: usize) -> bool {
    let functions = unsafe { FUNCTION_TABLE.as_ref().unwrap() };
    unsafe {
        wrapper::seh_invoke(
            functions.probe_for_read,
            target,
            length as u64,
            align as u64,
            0,
        )
    }
}

pub fn probe_write(target: u64, length: usize, align: usize) -> bool {
    let functions = unsafe { FUNCTION_TABLE.as_ref().unwrap() };
    unsafe {
        wrapper::seh_invoke(
            functions.probe_for_write,
            target,
            length as u64,
            align as u64,
            0,
        )
    }
}

pub fn probe_and_lock_pages(mdl: *const (), access_mode: KPROCESSOR_MODE, operation: u32) -> bool {
    /*
     * We must use MmProbeAndLockProcessPages instead of MmProbeAndLockPages as
     * MmProbeAndLockPages writes to the shaddow stack, which we do not support.
     * MmProbeAndLockProcessPages is identical when Process == PsGetCurrentProcess() therefore we're good here.
     */
    let functions = unsafe { FUNCTION_TABLE.as_ref().unwrap() };
    let current_process = unsafe { PsGetCurrentProcess() };

    unsafe {
        wrapper::seh_invoke(
            functions.mm_probe_and_lock_process_pages,
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
    let functions = unsafe { FUNCTION_TABLE.as_ref().unwrap() };
    unsafe {
        wrapper::seh_invoke(
            functions.memmove,
            target.as_mut_ptr() as u64,
            source,
            target.len() as u64,
            0,
        )
    }
}
