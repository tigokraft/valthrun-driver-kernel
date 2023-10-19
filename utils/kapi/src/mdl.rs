use winapi::{
    km::{
        ndis::PMDL,
        wdm::PIRP,
    },
    shared::ntdef::PVOID,
};

use crate::imports::GLOBAL_IMPORTS;

pub struct Mdl {
    inner: PMDL,
}

impl Mdl {
    pub fn from_raw(mdl: PMDL) -> Self {
        Self { inner: mdl }
    }

    pub fn allocate(
        address: PVOID,
        length: usize,
        secondary_buffer: bool,
        charge_quota: bool,
        irp: PIRP,
    ) -> Option<Self> {
        let imports = GLOBAL_IMPORTS.unwrap();
        let mdl = unsafe {
            (imports.IoAllocateMdl)(
                address as *mut _,
                length as u32,
                secondary_buffer,
                charge_quota,
                irp,
            )
        };
        if mdl.is_null() {
            return None;
        }

        Some(Self { inner: mdl })
    }

    pub fn mdl(&self) -> PMDL {
        self.inner
    }

    pub fn into_raw(mut self) -> PMDL {
        let value = self.inner;
        self.inner = core::ptr::null_mut();
        value
    }
}

impl Drop for Mdl {
    fn drop(&mut self) {
        if self.inner.is_null() {
            return;
        }

        let imports = GLOBAL_IMPORTS.unwrap();
        unsafe { (imports.IoFreeMdl)(self.inner) };
    }
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

// pub struct LockedVirtMem {
//     mdl: PMDL,
//     address: PVOID,
//     length: usize,
// }

// impl LockedVirtMem {
//     pub fn create(
//         address: u64,
//         length: usize,
//         access_mode: KPROCESSOR_MODE,
//         operation: u32,
//         cache: u32,
//     ) -> Option<Self> {
//         log::debug!("MDL");
//         let imports = GLOBAL_IMPORTS.unwrap();
//         let mdl = unsafe {
//             (imports.IoAllocateMdl)(
//                 address as PVOID,
//                 length as u32,
//                 false,
//                 false,
//                 core::ptr::null_mut(),
//             )
//         };
//         if mdl.is_null() {
//             return None;
//         }

//         log::debug!("P&L");
//         if !self::probe_and_lock_pages(mdl, access_mode, operation) {
//             unsafe {
//                 (imports.IoFreeMdl)(mdl);
//             }

//             return None;
//         }

//         log::debug!("MmMapLockedPagesSpecifyCache");
//         let address = unsafe {
//             (imports.MmMapLockedPagesSpecifyCache)(
//                 mdl,
//                 KPROCESSOR_MODE::KernelMode,
//                 cache,
//                 core::ptr::null_mut(),
//                 0,
//                 0,
//             )
//         };
//         // let address = unsafe {
//         //     MmGetSystemAddressForMdlSafe(mdl, 0)
//         // };

//         if address.is_null() {
//             unsafe {
//                 (imports.MmUnlockPages)(mdl);
//                 (imports.IoFreeMdl)(mdl);
//             }

//             return None;
//         }

//         Some(Self {
//             mdl,
//             length,
//             address,
//         })
//     }

//     pub fn memory(&self) -> &mut [u8] {
//         unsafe { core::slice::from_raw_parts_mut(self.address as *mut u8, self.length) }
//     }
// }

// impl Drop for LockedVirtMem {
//     fn drop(&mut self) {
//         let imports = GLOBAL_IMPORTS.unwrap();
//         unsafe {
//             //MmUnmapLockedPages(self.address, self.mdl);
//             (imports.MmUnlockPages)(self.mdl);
//             (imports.IoFreeMdl)(self.mdl);
//         }
//     }
// }
