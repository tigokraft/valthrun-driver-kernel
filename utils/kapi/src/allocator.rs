use core::{
    alloc::{
        AllocError,
        Allocator,
        GlobalAlloc,
        Layout,
    },
    ptr::NonNull,
};

use utils_imports::{
    dynamic_import_table,
    provider::SystemExport,
};
use winapi::{
    km::wdm::POOL_TYPE,
    shared::ntdef::PVOID,
};

pub const POOL_TAG: u32 = 0x123333;

type ExAllocatePoolWithTag =
    unsafe extern "system" fn(PoolType: POOL_TYPE, NumberOfBytes: usize, Tag: u32) -> PVOID;
type ExFreePoolWithTag = unsafe extern "system" fn(P: PVOID, Tag: u32);

type MmAllocateContiguousMemory =
    unsafe extern "system" fn(NumberOfBytes: usize, HighestAcceptableAddress: usize) -> PVOID;
type MmFreeContiguousMemory = unsafe extern "system" fn(P: PVOID);

dynamic_import_table! {
    imports IMPORTS_ALLOCATOR {
        pub ExAllocatePoolWithTag: ExAllocatePoolWithTag = SystemExport::new(obfstr!("ExAllocatePoolWithTag")),
        pub ExFreePoolWithTag: ExFreePoolWithTag = SystemExport::new(obfstr!("ExFreePoolWithTag")),

        pub MmAllocateContiguousMemory: MmAllocateContiguousMemory = SystemExport::new(obfstr!("MmAllocateContiguousMemory")),
        pub MmFreeContiguousMemory: MmFreeContiguousMemory = SystemExport::new(obfstr!("MmFreeContiguousMemory")),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ContiguousMemoryAllocator {
    highest_acceptable_address: Option<usize>,
}

impl ContiguousMemoryAllocator {
    pub const fn new(highest_acceptable_address: Option<usize>) -> Self {
        Self {
            highest_acceptable_address,
        }
    }
}

unsafe impl Allocator for ContiguousMemoryAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        #[allow(non_snake_case)]
        let MmAllocateContiguousMemory = match IMPORTS_ALLOCATOR.resolve() {
            Ok(table) => table.MmAllocateContiguousMemory,
            /*
             * Failed to find target import.
             * Alloc failed.
             */
            Err(_) => return Err(AllocError),
        };

        let result = unsafe {
            (MmAllocateContiguousMemory)(
                layout.size(),
                self.highest_acceptable_address.unwrap_or(usize::MAX),
            )
        } as *mut u8;

        NonNull::new(result)
            .ok_or(AllocError)
            .map(|data| NonNull::slice_from_raw_parts(data, layout.size()))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, _layout: core::alloc::Layout) {
        #[allow(non_snake_case)]
        let MmFreeContiguousMemory = IMPORTS_ALLOCATOR.unwrap().MmFreeContiguousMemory;
        (MmFreeContiguousMemory)(ptr.as_ptr() as PVOID);
    }
}

pub struct NonPagedAllocator;
unsafe impl GlobalAlloc for NonPagedAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        #[allow(non_snake_case)]
        let ExAllocatePoolWithTag = match IMPORTS_ALLOCATOR.resolve() {
            Ok(table) => table.ExAllocatePoolWithTag,
            /*
             * Failed to find target import.
             * Alloc failed.
             */
            Err(_) => return core::ptr::null_mut(),
        };

        (ExAllocatePoolWithTag)(POOL_TYPE::NonPagedPool, layout.size(), POOL_TAG) as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) {
        /*
         * If we can allocate data we *must* deallocate this data or panic
         * to avoid unwanted side effects.
         */
        #[allow(non_snake_case)]
        let ExFreePoolWithTag = IMPORTS_ALLOCATOR.unwrap().ExFreePoolWithTag;
        (ExFreePoolWithTag)(ptr as PVOID, POOL_TAG);
    }
}
