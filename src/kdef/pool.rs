//! Kernel Mode pools.

use winapi::{km::wdm::POOL_TYPE, shared::ntdef::PVOID};

#[allow(unused)]
extern "system" {
    /// Allocates pool memory of the specified type and tag.
    pub fn ExAllocatePoolWithTag(PoolType: POOL_TYPE, NumberOfBytes: usize, Tag: u32) -> PVOID;
    /// Deallocates a block of pool memory allocated with the specified tag.
    pub fn ExFreePoolWithTag(P: PVOID, Tag: u32);

    /// Allocates pool memory of the specified type.
    pub fn ExAllocatePool(PoolType: POOL_TYPE, NumberOfBytes: usize) -> PVOID;
    /// Deallocates a block of pool memory.
    pub fn ExFreePool(P: PVOID);

    pub fn ProbeForRead(address: *const (), length: usize, alignment: u32);
    pub fn ProbeForWrite(address: *mut (), length: usize, alignment: u32);
}
