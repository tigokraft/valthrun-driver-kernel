use winapi::shared::ntdef::{
    HANDLE,
    PVOID,
};

#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
pub struct _SYSTEM_MODULE_ENTRY {
    pub Section: HANDLE,
    pub MappedBase: PVOID,
    pub ImageBase: PVOID,
    pub ImageSize: u32,
    pub Flags: u32,
    pub LoadOrderIndex: u16,
    pub InitOrderIndex: u16,
    pub LoadCount: u16,
    pub OffsetToFileName: u16,
    pub FullPathName: [u8; 256],
}

#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
pub struct _SYSTEM_MODULE_INFORMATION {
    Count: u32,
    Module: [_SYSTEM_MODULE_ENTRY; 0],
}

impl _SYSTEM_MODULE_INFORMATION {
    pub fn modules(&self) -> &[_SYSTEM_MODULE_ENTRY] {
        unsafe {
            let ptr = core::mem::transmute::<_, *const _SYSTEM_MODULE_ENTRY>(&self.Module);
            core::slice::from_raw_parts(ptr, self.Count as usize)
        }
    }
}

#[allow(non_upper_case_globals)]
pub const SystemModuleInformation: u32 = 0x0B;
