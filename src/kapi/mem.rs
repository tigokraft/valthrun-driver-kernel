use winapi::shared::ntdef::UNICODE_STRING;

use crate::kdef::MmGetSystemRoutineAddress;

use super::{UnicodeStringEx, seh};

pub fn probe_read(target: u64, length: usize, align: usize) -> bool {
    let target_fn = unsafe {
        let name = UNICODE_STRING::from_bytes(obfstr::wide!("ProbeForRead"));
        MmGetSystemRoutineAddress(&name)
    };

    if target_fn.is_null() {
        log::warn!("Missing ProbeForRead");
        return false;
    }

    unsafe {
        seh::seh_invoke(target_fn as u64, target, length as u64, align as u64, 0)
    }
}

pub fn probe_write(target: u64, length: usize, align: usize) -> bool {
    let target_fn = unsafe {
        let name = UNICODE_STRING::from_bytes(obfstr::wide!("ProbeForWrite"));
        MmGetSystemRoutineAddress(&name)
    };

    if target_fn.is_null() {
        log::warn!("Missing ProbeForWrite");
        return false;
    }

    unsafe {
        seh::seh_invoke(target_fn as u64, target, length as u64, align as u64, 0)
    }
}

/// Copy memory from source into target.
/// Returns false on failure.
pub fn safe_copy(target: &mut [u8], source: u64) -> bool {
    let target_fn = unsafe {
        let name = UNICODE_STRING::from_bytes(obfstr::wide!("memmove"));
        MmGetSystemRoutineAddress(&name)
    };

    if target_fn.is_null() {
        log::warn!("Missing memmove");
        return false;
    }

    unsafe {
        seh::seh_invoke(target_fn as u64, target.as_mut_ptr() as u64, source, target.len() as u64, 0)
    }
}