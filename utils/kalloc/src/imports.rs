use utils_imports::{
    dynamic_import_table,
    provider::SystemExport,
};
use winapi::{
    km::wdm::POOL_TYPE,
    shared::ntdef::PVOID,
};

type ExAllocatePoolWithTag =
    unsafe extern "system" fn(PoolType: POOL_TYPE, NumberOfBytes: usize, Tag: u32) -> PVOID;
type ExFreePoolWithTag = unsafe extern "system" fn(P: PVOID, Tag: u32);

type MmAllocateContiguousMemory =
    unsafe extern "system" fn(NumberOfBytes: usize, HighestAcceptableAddress: usize) -> PVOID;
type MmFreeContiguousMemory = unsafe extern "system" fn(P: PVOID);

dynamic_import_table! {
    pub(crate) imports IMPORTS_ALLOCATOR {
        pub ExAllocatePoolWithTag: ExAllocatePoolWithTag = SystemExport::new(obfstr!("ExAllocatePoolWithTag")),
        pub ExFreePoolWithTag: ExFreePoolWithTag = SystemExport::new(obfstr!("ExFreePoolWithTag")),

        pub MmAllocateContiguousMemory: MmAllocateContiguousMemory = SystemExport::new(obfstr!("MmAllocateContiguousMemory")),
        pub MmFreeContiguousMemory: MmFreeContiguousMemory = SystemExport::new(obfstr!("MmFreeContiguousMemory")),
    }
}
