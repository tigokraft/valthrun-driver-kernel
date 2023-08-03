use winapi::shared::ntdef::NTSTATUS;

use crate::kapi::NTStatusEx;

#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
pub struct _OSVERSIONINFOEXW {
    pub dwOSVersionInfoSize: u32,
    pub dwMajorVersion: u32,
    pub dwMinorVersion: u32,
    pub dwBuildNumber: u32,
    pub dwPlatformId: u32,
 
    pub szCSDVersion: [u16; 128],
    pub wServicePackMajor: u16,
    pub wServicePackMinor: u16,
    pub wSuiteMask: u16,
 
    pub wProductType: u8,
    pub wReserved: u8
}


extern "system" {
    fn RtlGetVersion(info: &mut _OSVERSIONINFOEXW) -> NTSTATUS;
}

pub static OS_VERSION_INFO: _OSVERSIONINFOEXW = _OSVERSIONINFOEXW {
    dwOSVersionInfoSize: 0, 
    dwMajorVersion: 0, 
    dwMinorVersion: 0, 
    dwBuildNumber: 0, 
    dwPlatformId: 0, 
    szCSDVersion: [0; 128], 
    wServicePackMajor: 0, 
    wServicePackMinor: 0, 
    wSuiteMask: 0, 
    wProductType: 0, 
    wReserved: 0
};

pub fn initialize_os_info() -> anyhow::Result<(), NTSTATUS> {
    #[allow(mutable_transmutes)]
    let mut info = unsafe { core::mem::transmute::<_, &mut _>(&OS_VERSION_INFO) };
    unsafe { RtlGetVersion(&mut info).ok() }
}