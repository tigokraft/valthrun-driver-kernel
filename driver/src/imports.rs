#![allow(non_snake_case)]
#![allow(dead_code)]

// use kdef::{
//     OBJECT_NAME_INFORMATION,
//     POBJECT_TYPE,
//     _KAPC_STATE,
//     _OB_CALLBACK_REGISTRATION,
//     _PEB,
// };
// use winapi::{
//     km::{
//         ndis::PMDL,
//         wdm::{
//             DEVICE_OBJECT,
//             DEVICE_TYPE,
//             DRIVER_OBJECT,
//             IO_PRIORITY::KPRIORITY_BOOST,
//             KPROCESSOR_MODE,
//             PDEVICE_OBJECT,
//             PEPROCESS,
//             PETHREAD,
//             PIRP,
//         },
//     },
//     shared::{
//         guiddef::LPCGUID,
//         ntdef::{
//             BOOLEAN,
//             CCHAR,
//             HANDLE,
//             KIRQL,
//             NTSTATUS,
//             PCUNICODE_STRING,
//             PCVOID,
//             PHANDLE,
//             POBJECT_ATTRIBUTES,
//             PVOID,
//             UNICODE_STRING,
//         },
//     },
//     um::winnt::{
//         ACCESS_MASK,
//         PIMAGE_NT_HEADERS,
//     },
// };

use kdef::_OB_CALLBACK_REGISTRATION;
use lazy_link::lazy_link;
use winapi::{
    km::wdm::{
        KPROCESSOR_MODE,
        PDEVICE_OBJECT,
        PEPROCESS,
    },
    shared::ntdef::{
        NTSTATUS,
        PCVOID,
        PVOID,
    },
    um::winnt::OSVERSIONINFOEXW,
};

pub const BCRYPT_RNG_USE_ENTROPY_IN_BUFFER: u32 = 0x00000001;
pub const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x00000002;

#[lazy_link(resolver = "kapi_kmodule::resolve_import")]
extern "C" {
    pub fn IoRegisterShutdownNotification(DeviceObject: PDEVICE_OBJECT) -> NTSTATUS;
    pub fn IoUnregisterShutdownNotification(DeviceObject: PDEVICE_OBJECT);

    pub fn ObRegisterCallbacks(
        CallbackRegistration: *const _OB_CALLBACK_REGISTRATION,
        RegistrationHandle: *mut PVOID,
    ) -> NTSTATUS;
    pub fn ObUnRegisterCallbacks(RegistrationHandle: PVOID);

    pub fn KeQueryTimeIncrement() -> u32;
    pub fn KeQuerySystemTimePrecise(CurrentTime: *mut u64) -> ();

    pub fn RtlRandomEx(Seed: *mut u32) -> u32;
    pub fn RtlGetVersion(info: *mut OSVERSIONINFOEXW) -> NTSTATUS;

    pub fn ExGetSystemFirmwareTable(
        FirmwareTableProviderSignature: u32,
        FirmwareTableID: u32,
        FirmwareTableBuffer: PVOID,
        BufferLength: u32,
        ReturnLength: *mut u32,
    ) -> NTSTATUS;

    pub fn KeBugCheck(code: u32) -> !;

    pub fn MmCopyVirtualMemory(
        FromProcess: PEPROCESS,
        FromAddress: PCVOID,
        ToProcess: PEPROCESS,
        ToAddress: PVOID,
        BufferSize: usize,
        PreviousMode: KPROCESSOR_MODE,
        NumberOfBytesCopied: *mut usize,
    ) -> NTSTATUS;
}
