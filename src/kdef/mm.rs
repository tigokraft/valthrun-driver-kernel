use winapi::{
    km::wdm::{
        KPROCESSOR_MODE,
        PIRP,
    },
    shared::ntdef::PVOID,
};

use crate::wsk::sys::PMDL;

#[allow(dead_code)]
extern "system" {
    pub fn IoAllocateMdl(
        VirtualAddress: PVOID,
        Length: u32,
        SecondaryBuffer: bool,
        ChargeQuota: bool,
        Irp: PIRP,
    ) -> PMDL;

    pub fn IoFreeMdl(MemoryDescriptorList: PMDL);

    pub fn MmProbeAndLockPages(
        MemoryDescriptorList: PMDL,
        AccessMode: KPROCESSOR_MODE,
        Operation: u32,
    );

    pub fn MmUnlockPages(MemoryDescriptorList: PMDL);

    pub fn MmMapLockedPagesSpecifyCache(
        MemoryDescriptorList: PMDL,
        AccessMode: KPROCESSOR_MODE,
        CacheType: u32,
        RequestedAddress: PVOID,
        BugCheckOnFailure: u32,
        Priority: u32,
    ) -> PVOID;

    pub fn MmGetSystemAddressForMdlSafe(MemoryDescriptorList: PMDL, Priority: u32) -> PVOID;

    pub fn MmUnmapLockedPages(BaseAddress: PVOID, MemoryDescriptorList: PMDL);
}
