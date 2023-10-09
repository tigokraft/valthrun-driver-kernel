use core::cell::SyncUnsafeCell;

use obfstr::obfstr;
use winapi::{
    km::{
        ndis::PMDL,
        wdm::{
            KPROCESSOR_MODE,
            PIRP,
        },
    },
    shared::ntdef::{
        PVOID,
        UNICODE_STRING,
    },
};

use super::{
    seh,
    UnicodeStringEx,
};
use crate::kdef::MmGetSystemRoutineAddress;
#[derive(Default)]
struct MemFunctions {
    probe_for_read: u64,
    probe_for_write: u64,
    probe_and_lock: u64,
    memmove: u64,
}
static MEM_FUNCTIONS: SyncUnsafeCell<MemFunctions> = SyncUnsafeCell::new(MemFunctions {
    memmove: 0,
    probe_for_read: 0,
    probe_for_write: 0,
    probe_and_lock: 0,
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

    function_table.probe_and_lock = unsafe {
        let name = UNICODE_STRING::from_bytes(obfstr::wide!("MmProbeAndLockPages"));
        MmGetSystemRoutineAddress(&name) as u64
    };
    if function_table.probe_and_lock == 0 {
        anyhow::bail!("{}", obfstr!("failed to resolve MmProbeAndLockPages"))
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

pub fn probe_and_lock_pages(mdl: PMDL, access_mode: KPROCESSOR_MODE, operation: u32) -> bool {
    let target_fn = unsafe { &*MEM_FUNCTIONS.get() }.probe_and_lock;
    if target_fn == 0 {
        return false;
    }

    unsafe {
        seh::seh_invoke(
            target_fn,
            mdl as u64,
            access_mode as u64,
            operation as u64,
            0,
        )
    }
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

extern "system" {
    fn IoAllocateMdl(
        VirtualAddress: PVOID,
        Length: u32,
        SecondaryBuffer: bool,
        ChargeQuota: bool,
        Irp: PIRP,
    ) -> PMDL;

    fn IoFreeMdl(MemoryDescriptorList: PMDL);

    fn MmProbeAndLockPages(MemoryDescriptorList: PMDL, AccessMode: KPROCESSOR_MODE, Operation: u32);

    fn MmUnlockPages(MemoryDescriptorList: PMDL);

    fn MmMapLockedPagesSpecifyCache(
        MemoryDescriptorList: PMDL,
        AccessMode: KPROCESSOR_MODE,
        CacheType: u32,
        RequestedAddress: PVOID,
        BugCheckOnFailure: u32,
        Priority: u32,
    ) -> PVOID;

    fn MmGetSystemAddressForMdlSafe(MemoryDescriptorList: PMDL, Priority: u32) -> PVOID;

    fn MmUnmapLockedPages(BaseAddress: PVOID, MemoryDescriptorList: PMDL);
}

pub const MCT_NON_CACHED: u32 = 0x00;
pub const MCT_CACHED: u32 = 0x01;
pub const MCT_WRITE_COMBINED: u32 = 0x02;
pub const MCT_HARDWARE_COHERENT_CACHED: u32 = 0x03;
pub const MCT_NON_CACHED_UNORDERED: u32 = 0x04;
pub const MCT_USWC_CACHED: u32 = 0x05;
pub const MCT_MAXIMUM_CACHE_TYPE: u32 = 0x06;
pub const MCT_NOT_MAPPED: u32 = 0x07;

pub const IO_READ_ACCESS: u32 = 0x00;
pub const IO_WRITE_ACCESS: u32 = 0x01;
pub const IO_MODIFY_ACCESS: u32 = 0x02;

pub struct LockedVirtMem {
    mdl: PMDL,
    address: PVOID,
    length: usize,
}

impl LockedVirtMem {
    pub fn create(
        address: u64,
        length: usize,
        access_mode: KPROCESSOR_MODE,
        operation: u32,
        cache: u32,
    ) -> Option<Self> {
        let access_mode2 = match &access_mode {
            KPROCESSOR_MODE::KernelMode => KPROCESSOR_MODE::KernelMode,
            KPROCESSOR_MODE::UserMode => KPROCESSOR_MODE::UserMode,
        };

        log::debug!("MDL");
        let mdl = unsafe {
            IoAllocateMdl(
                address as PVOID,
                length as u32,
                false,
                false,
                core::ptr::null_mut(),
            )
        };
        if mdl.is_null() {
            return None;
        }

        log::debug!("P&L");
        if !self::probe_and_lock_pages(mdl, access_mode, operation) {
            unsafe {
                IoFreeMdl(mdl);
            }

            return None;
        }

        log::debug!("MmMapLockedPagesSpecifyCache");
        let address = unsafe {
            MmMapLockedPagesSpecifyCache(
                mdl,
                KPROCESSOR_MODE::KernelMode,
                cache,
                core::ptr::null_mut(),
                0,
                0,
            )
        };
        // let address = unsafe {
        //     MmGetSystemAddressForMdlSafe(mdl, 0)
        // };

        if address.is_null() {
            unsafe {
                MmUnlockPages(mdl);
                IoFreeMdl(mdl);
            }

            return None;
        }

        Some(Self {
            mdl,
            length,
            address,
        })
    }

    pub fn memory(&self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.address as *mut u8, self.length) }
    }
}

impl Drop for LockedVirtMem {
    fn drop(&mut self) {
        unsafe {
            //MmUnmapLockedPages(self.address, self.mdl);
            MmUnlockPages(self.mdl);
            IoFreeMdl(self.mdl);
        }
    }
}
