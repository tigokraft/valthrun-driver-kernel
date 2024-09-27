use core::cell::SyncUnsafeCell;

use anyhow::anyhow;
use kapi::NTStatusEx;
use winapi::um::winnt::OSVERSIONINFOEXW;

use crate::imports::RtlGetVersion;

static OS_VERSION_INFO: SyncUnsafeCell<OSVERSIONINFOEXW> = SyncUnsafeCell::new(OSVERSIONINFOEXW {
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

pub fn os_info() -> &'static OSVERSIONINFOEXW {
    unsafe { &*OS_VERSION_INFO.get() }
}

pub fn initialize_os_info() -> anyhow::Result<()> {
    let info = unsafe { &mut *OS_VERSION_INFO.get() };
    unsafe { RtlGetVersion(info).ok().map_err(|err| anyhow!("{:X}", err)) }
}
