use core::{
    ptr,
    slice,
};

use kdef::_MDL;
use winapi::{
    ctypes::c_void,
    km::wdm::{
        KPROCESSOR_MODE,
        PIRP,
    },
    shared::ntdef::PVOID,
};

use crate::imports::GLOBAL_IMPORTS;

pub struct Mdl {
    handle: *mut _MDL,
}

impl Mdl {
    pub fn from_raw(mdl: *mut _MDL) -> Self {
        Self { handle: mdl }
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

        Some(Self { handle: mdl })
    }

    pub fn raw_mdl(&self) -> *mut _MDL {
        self.handle
    }

    pub fn into_raw(mut self) -> *mut _MDL {
        let value = self.handle;
        self.handle = core::ptr::null_mut();
        value
    }

    pub fn lock(self, access_mode: KPROCESSOR_MODE, operation: u32) -> Result<LockedMDL, Mdl> {
        LockedMDL::try_lock(self, access_mode, operation)
    }

    pub fn len(&self) -> usize {
        unsafe { &*self.raw_mdl() }.byte_count as usize
    }
}

impl Drop for Mdl {
    fn drop(&mut self) {
        if self.handle.is_null() {
            return;
        }

        let imports = GLOBAL_IMPORTS.unwrap();
        unsafe { (imports.IoFreeMdl)(self.handle) };
    }
}

pub enum PagePriority {
    LOW,
    NORMAL,
    HIGH,
}

impl PagePriority {
    pub fn ordinal(&self) -> u32 {
        match self {
            Self::LOW => 0,
            Self::NORMAL => 16,
            Self::HIGH => 32,
        }
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

pub struct LockedMDL {
    handle: Option<Mdl>,
}

impl LockedMDL {
    fn try_lock(mdl: Mdl, access_mode: KPROCESSOR_MODE, operation: u32) -> Result<Self, Mdl> {
        if !seh::probe_and_lock_pages(mdl.raw_mdl() as *const (), access_mode, operation) {
            Err(mdl)
        } else {
            Ok(Self { handle: Some(mdl) })
        }
    }

    pub fn mdl(&self) -> &Mdl {
        self.handle.as_ref().unwrap()
    }

    pub fn unlock(mut self) -> Mdl {
        self.do_unlock();
        self.handle.take().unwrap()
    }

    fn do_unlock(&mut self) {
        let imports = GLOBAL_IMPORTS.unwrap();
        unsafe { (imports.MmUnlockPages)(self.mdl().raw_mdl()) };
    }

    pub fn map(
        self,
        access_mode: KPROCESSOR_MODE,
        cache_type: u32,
        mut requested_address: Option<usize>,
        priority: PagePriority,
    ) -> Result<MappedLockedMDL, LockedMDL> {
        let imports = GLOBAL_IMPORTS.unwrap();
        let requested_address = requested_address
            .as_mut()
            .map(|r| r as *mut _ as *mut c_void)
            .unwrap_or_else(ptr::null_mut);

        let address = unsafe {
            (imports.MmMapLockedPagesSpecifyCache)(
                self.mdl().raw_mdl(),
                access_mode,
                cache_type,
                requested_address,
                0,
                priority.ordinal(),
            )
        };

        if address.is_null() {
            Err(self)
        } else {
            Ok(MappedLockedMDL { mdl: self, address })
        }
    }
}

impl Drop for LockedMDL {
    fn drop(&mut self) {
        if self.handle.is_some() {
            self.do_unlock();
        }
    }
}

pub struct MappedLockedMDL {
    mdl: LockedMDL,
    address: PVOID,
}

impl MappedLockedMDL {
    pub fn address(&self) -> PVOID {
        self.address
    }

    pub fn len(&self) -> usize {
        self.mdl.mdl().len()
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.address as *const u8, self.len()) }
    }

    pub fn as_slice_mut(&self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.address as *mut u8, self.len()) }
    }
}

impl Drop for MappedLockedMDL {
    fn drop(&mut self) {
        let imports = GLOBAL_IMPORTS.unwrap();
        unsafe { (imports.MmUnmapLockedPages)(self.address, self.mdl.mdl().raw_mdl()) };
    }
}
