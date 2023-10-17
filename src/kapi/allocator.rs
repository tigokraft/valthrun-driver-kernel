use core::alloc::GlobalAlloc;

use winapi::{
    km::wdm::POOL_TYPE,
    shared::ntdef::PVOID,
};

use crate::util::imports::LLSystemExport;

pub const POOL_TAG: u32 = 0x123333;

type ExAllocatePoolWithTag = unsafe extern "system" fn(PoolType: POOL_TYPE, NumberOfBytes: usize, Tag: u32) -> PVOID;
type ExFreePoolWithTag = unsafe extern "system" fn(P: PVOID, Tag: u32);

#[allow(non_upper_case_globals)]
static IMPORT_ExAllocatePoolWithTag: LLSystemExport<ExAllocatePoolWithTag> = LLSystemExport::new("ExAllocatePoolWithTag");

#[allow(non_upper_case_globals)]
static IMPORT_ExFreePoolWithTag: LLSystemExport<ExFreePoolWithTag> = LLSystemExport::new("ExFreePoolWithTag");

struct NonPagedAllocator;
unsafe impl GlobalAlloc for NonPagedAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        #[allow(non_snake_case)]
        let ExAllocatePoolWithTag = match IMPORT_ExAllocatePoolWithTag.resolve() {
            Some(func) => func,
            /*
             * Failed to find target import.
             * Alloc failed.
             */
            None => return core::ptr::null_mut()
        };

        (ExAllocatePoolWithTag)(POOL_TYPE::NonPagedPool, layout.size(), POOL_TAG) as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) {
        /*
         * If we can allocate data we *must* deallocate this data or panic
         * to avoid unwanted side effects.
         */
        #[allow(non_snake_case)]
        let ExFreePoolWithTag = IMPORT_ExFreePoolWithTag.resolve().unwrap();
        (ExFreePoolWithTag)(ptr as PVOID, POOL_TAG);
    }
}

#[global_allocator]
static GLOBAL_ALLOC: NonPagedAllocator = NonPagedAllocator;
