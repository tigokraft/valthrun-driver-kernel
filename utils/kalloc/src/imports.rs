#![allow(non_snake_case)]

use lazy_link::lazy_link;
use winapi::{
    km::wdm::POOL_TYPE,
    shared::ntdef::PVOID,
};

/*
 * We have to use utils_imports::resolve_system here to ensure
 * we do the bare minimum when resolving these functions as if we would allocate any memory,
 * we would end up in an infinity loop
 */
#[lazy_link(resolver = "utils_imports::resolve_system")]
extern "system" {
    pub fn ExAllocatePoolWithTag(PoolType: POOL_TYPE, NumberOfBytes: usize, Tag: u32) -> PVOID;
    pub fn ExFreePoolWithTag(P: PVOID, Tag: u32);

    pub fn MmAllocateContiguousMemory(
        NumberOfBytes: usize,
        HighestAcceptableAddress: usize,
    ) -> PVOID;
    pub fn MmFreeContiguousMemory(P: PVOID);
}
