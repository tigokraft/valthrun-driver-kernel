use core::cell::SyncUnsafeCell;

use anyhow::anyhow;
use winapi::shared::ntdef::NTSTATUS;

use crate::{
    dynamic_import_table,
    kapi::NTStatusEx,
    util::imports::SystemExport,
};

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
    pub wReserved: u8,
}

static OS_VERSION_INFO: SyncUnsafeCell<_OSVERSIONINFOEXW> =
    SyncUnsafeCell::new(_OSVERSIONINFOEXW {
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
        wReserved: 0,
    });

pub fn os_info() -> &'static _OSVERSIONINFOEXW {
    unsafe { &*OS_VERSION_INFO.get() }
}

// Using a private import table here, as the global import table might not be initialized yet.
type RtlGetVersion = unsafe extern "C" fn(info: &mut _OSVERSIONINFOEXW) -> NTSTATUS;
dynamic_import_table! {
    imports IMPORTS {
        pub RtlGetVersion: RtlGetVersion = SystemExport::new(obfstr::wide!("RtlGetVersion")),
    }
}

pub fn initialize_os_info() -> anyhow::Result<()> {
    let imports = IMPORTS
        .resolve()
        .map_err(|err| anyhow!("{}: {}", "failed to resolve imports", err))?;

    let mut info = unsafe { &mut *OS_VERSION_INFO.get() };
    unsafe {
        (imports.RtlGetVersion)(&mut info)
            .ok()
            .map_err(|err| anyhow!("{:X}", err))
    }
}
