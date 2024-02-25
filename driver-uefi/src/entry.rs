use alloc::{
    slice,
    string::ToString,
};
use core::ptr;

use anyhow::{
    anyhow,
    Context,
};
use kapi::{
    Mdl,
    PagePriority,
    IO_MODIFY_ACCESS,
    MCT_NON_CACHED,
};
use obfstr::obfstr;
use winapi::{
    km::wdm::{
        DRIVER_OBJECT,
        KPROCESSOR_MODE,
    },
    shared::ntdef::{
        NTSTATUS,
        PVOID,
        UNICODE_STRING,
    },
};

#[no_mangle]
static _ENTRY_BYTES: [u8; 0x20] = [0; 0x20];

pub type FnDriverEntry = extern "system" fn(*mut DRIVER_OBJECT, *const UNICODE_STRING) -> NTSTATUS;

#[inline(never)]
pub fn has_custom_entry() -> bool {
    let first_byte = unsafe { ptr::read_volatile(_ENTRY_BYTES.as_ptr()) };
    first_byte > 0
}

pub fn restore_original_entry(entry_point: FnDriverEntry) -> anyhow::Result<()> {
    let mdl = Mdl::allocate(
        entry_point as PVOID,
        _ENTRY_BYTES.len(),
        false,
        false,
        ptr::null_mut(),
    )
    .with_context(|| obfstr!("failed to allocate MDL").to_string())?;

    let mdl = mdl
        .lock(KPROCESSOR_MODE::KernelMode, IO_MODIFY_ACCESS)
        .map_err(|_| anyhow!("{}", obfstr!("failed to lock MDL")))?;

    let mdl = mdl
        .map(
            KPROCESSOR_MODE::KernelMode,
            MCT_NON_CACHED,
            None,
            PagePriority::HIGH,
        )
        .map_err(|_| anyhow!("{}", obfstr!("failed to map MDL")))?;

    let entry_bytes = unsafe { ptr::read_volatile(&_ENTRY_BYTES) };
    let entry_slice =
        unsafe { slice::from_raw_parts_mut(mdl.address() as *mut u8, entry_bytes.len()) };

    /* Restore the original driver entry */
    entry_slice.copy_from_slice(&entry_bytes);

    Ok(())
}
