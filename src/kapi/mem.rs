use core::cell::SyncUnsafeCell;

use obfstr::obfstr;
use winapi::shared::ntdef::UNICODE_STRING;

use super::{
    seh,
    UnicodeStringEx,
};
use crate::kdef::MmGetSystemRoutineAddress;
#[derive(Default)]
struct MemFunctions {
    probe_for_read: u64,
    probe_for_write: u64,
    memmove: u64,
}
static MEM_FUNCTIONS: SyncUnsafeCell<MemFunctions> = SyncUnsafeCell::new(MemFunctions {
    memmove: 0,
    probe_for_read: 0,
    probe_for_write: 0,
});

pub fn init() -> anyhow::Result<()> {
    let function_table = unsafe { &mut *MEM_FUNCTIONS.get() };

    function_table.probe_for_read = unsafe {
        let name = UNICODE_STRING::from_bytes(obfstr::wide!("ProbeForRead"));
        MmGetSystemRoutineAddress(&name) as u64
    };
    if function_table.probe_for_read == 0 {
        anyhow::bail!("{}", obfstr!("failed to resolve ProbeForRead"))
    }

    function_table.probe_for_write = unsafe {
        let name = UNICODE_STRING::from_bytes(obfstr::wide!("ProbeForWrite"));
        MmGetSystemRoutineAddress(&name) as u64
    };
    if function_table.probe_for_write == 0 {
        anyhow::bail!("{}", obfstr!("failed to resolve ProbeForWrite"))
    }

    function_table.memmove = unsafe {
        let name = UNICODE_STRING::from_bytes(obfstr::wide!("memmove"));
        MmGetSystemRoutineAddress(&name) as u64
    };
    if function_table.memmove == 0 {
        anyhow::bail!("{}", obfstr!("failed to resolve memmove"))
    }

    Ok(())
}

pub fn probe_read(target: u64, length: usize, align: usize) -> bool {
    let target_fn = unsafe { &*MEM_FUNCTIONS.get() }.probe_for_read;
    if target_fn == 0 {
        return false;
    }

    unsafe { seh::seh_invoke(target_fn, target, length as u64, align as u64, 0) }
}

pub fn probe_write(target: u64, length: usize, align: usize) -> bool {
    let target_fn = unsafe { &*MEM_FUNCTIONS.get() }.probe_for_write;
    if target_fn == 0 {
        return false;
    }

    unsafe { seh::seh_invoke(target_fn, target, length as u64, align as u64, 0) }
}

/// Copy memory from source into target.
/// Returns false on failure.
pub fn safe_copy(target: &mut [u8], source: u64) -> bool {
    let target_fn = unsafe { &*MEM_FUNCTIONS.get() }.memmove;
    if target_fn == 0 {
        return false;
    }

    unsafe {
        seh::seh_invoke(
            target_fn,
            target.as_mut_ptr() as u64,
            source,
            target.len() as u64,
            0,
        )
    }
}
