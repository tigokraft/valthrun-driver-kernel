#![allow(non_snake_case)]

use lazy_link::lazy_link;
use winapi::{
    km::wdm::POOL_TYPE,
    shared::ntdef::PVOID,
};

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
