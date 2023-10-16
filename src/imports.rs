#![allow(unused)]

use winapi::{
    km::wdm::{
        DEVICE_TYPE,
        DRIVER_OBJECT,
        KPROCESSOR_MODE,
        PDEVICE_OBJECT,
        PEPROCESS,
        PETHREAD,
    },
    shared::{
        guiddef::LPCGUID,
        ntdef::{
            BOOLEAN,
            HANDLE,
            NTSTATUS,
            PCUNICODE_STRING,
            PHANDLE,
            POBJECT_ATTRIBUTES,
            PVOID,
            UNICODE_STRING,
        },
    }, um::winnt::ACCESS_MASK,
};

use crate::{
    dynamic_import_table,
    kdef::{
        _KAPC_STATE,
        _PEB, OBJECT_NAME_INFORMATION, POBJECT_TYPE,
    },
    util::imports::SystemExport,
    wsk::sys::{
        IN6_ADDR,
        IN_ADDR,
    },
};

pub const BCRYPT_RNG_USE_ENTROPY_IN_BUFFER: u32 = 0x00000001;
pub const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x00000002;

type BCryptGenRandom = unsafe extern "C" fn(
    hAlgorithm: *mut (),
    pbBuffer: *mut u8,
    cbBuffer: u32,
    dwFlags: u32,
) -> NTSTATUS;

type RtlIpv4AddressToStringExA = unsafe extern "C" fn(
    Address: &IN_ADDR,
    Port: u16,
    Buffer: *mut u8,
    BufferLength: &mut u32,
) -> NTSTATUS;

type RtlIpv6AddressToStringExA = unsafe extern "C" fn(
    Address: &IN6_ADDR,
    ScopeId: u32,
    Port: u16,
    Buffer: *mut u8,
    BufferLength: &mut u32,
) -> NTSTATUS;

type RtlRandomEx = unsafe extern "C" fn(Seed: *mut u32) -> u32;

type PsGetCurrentThread = unsafe extern "C" fn() -> PETHREAD;
type PsGetCurrentProcess = unsafe extern "C" fn() -> PEPROCESS;
type PsGetProcessId = unsafe extern "C" fn(process: PEPROCESS) -> i32;
type PsGetProcessPeb = unsafe extern "C" fn(process: PEPROCESS) -> *const _PEB;
type PsLookupProcessByProcessId =
    unsafe extern "C" fn(process_id: i32, process: *mut PEPROCESS) -> NTSTATUS;
type PsGetProcessImageFileName = unsafe extern "C" fn(Process: PEPROCESS) -> *const i8;
type PsCreateSystemThread = unsafe extern "C" fn(
    ThreadHandle: PHANDLE,
    DesiredAccess: u32,
    ObjectAttributes: POBJECT_ATTRIBUTES,
    ProcessHandle: HANDLE,
    ClientId: *mut u32,
    StartRoutine: extern "C" fn(PVOID) -> (),
    StartContext: PVOID,
) -> NTSTATUS;

type KeQuerySystemTimePrecise = unsafe extern "C" fn(CurrentTime: *mut u64) -> ();
type KeQueryTimeIncrement = unsafe extern "C" fn() -> u32;
type KeStackAttachProcess =
    unsafe extern "C" fn(process: PEPROCESS, apc_state: *mut _KAPC_STATE) -> ();
type KeUnstackDetachProcess = unsafe extern "C" fn(apc_state: *mut _KAPC_STATE) -> ();
pub type KeWaitForSingleObject = unsafe extern "C" fn(
    Object: PVOID,
    WaitReason: u32,
    WaitMode: KPROCESSOR_MODE,
    Alertable: bool,
    Timeout: *const u32,
) -> NTSTATUS;
type KeDelayExecutionThread = unsafe extern "C" fn(
    WaitMode: KPROCESSOR_MODE,
    Alertable: bool,
    Interval: *const u64,
) -> NTSTATUS;

type MmGetSystemRoutineAddress =
    unsafe extern "C" fn(system_routine_name: *const UNICODE_STRING) -> PVOID;

type ZwClose = unsafe extern "C" fn(Handle: HANDLE) -> NTSTATUS;

type IoCreateDeviceSecure = unsafe extern "system" fn(
    DriverObject: *mut DRIVER_OBJECT,
    DeviceExtensionSize: u32,
    DeviceName: PCUNICODE_STRING,
    DeviceType: DEVICE_TYPE,
    DeviceCharacteristics: u32,
    Exclusive: BOOLEAN,
    DefaultSDDLString: PCUNICODE_STRING,
    DeviceClassGuid: LPCGUID,
    DeviceObject: *mut PDEVICE_OBJECT,
) -> NTSTATUS;

type ObfDereferenceObject = unsafe extern "system" fn(object: PVOID);
type ObfReferenceObject = unsafe extern "system" fn(object: PVOID);
type ObQueryNameString = unsafe extern "system" fn(
    Object: PVOID,
    ObjectNameInfo: *mut OBJECT_NAME_INFORMATION,
    Length: u32,
    ReturnLength: &mut u32,
) -> NTSTATUS;
type ObReferenceObjectByName = unsafe extern "system" fn(
    ObjectName: PCUNICODE_STRING,
    Attributes: u32,
    AccessState: *mut (),
    DesiredAccess: ACCESS_MASK,
    ObjectType: POBJECT_TYPE,
    AccessMode: KPROCESSOR_MODE,
    ParseContext: PVOID,
    Object: PVOID,
) -> NTSTATUS;
type ObReferenceObjectByHandle = unsafe extern "system" fn(
    Handle: HANDLE,
    DesiredAccess: ACCESS_MASK,
    ObjectType: POBJECT_TYPE,
    AccessMode: KPROCESSOR_MODE,
    Object: PVOID,
    HandleInformation: PVOID,
) -> NTSTATUS;

dynamic_import_table! {
    pub imports GLOBAL_IMPORTS {
        pub RtlIpv4AddressToStringExA: RtlIpv4AddressToStringExA = SystemExport::new(obfstr::wide!("RtlIpv4AddressToStringExA")),
        pub RtlIpv6AddressToStringExA: RtlIpv6AddressToStringExA = SystemExport::new(obfstr::wide!("RtlIpv6AddressToStringExA")),

        pub KeQuerySystemTimePrecise: KeQuerySystemTimePrecise = SystemExport::new(obfstr::wide!("KeQuerySystemTimePrecise")),
        pub KeQueryTimeIncrement: KeQueryTimeIncrement = SystemExport::new(obfstr::wide!("KeQueryTimeIncrement")),
        pub KeStackAttachProcess: KeStackAttachProcess = SystemExport::new(obfstr::wide!("KeStackAttachProcess")),
        pub KeUnstackDetachProcess: KeUnstackDetachProcess = SystemExport::new(obfstr::wide!("KeUnstackDetachProcess")),
        pub KeWaitForSingleObject: KeWaitForSingleObject = SystemExport::new(obfstr::wide!("KeWaitForSingleObject")),
        pub KeDelayExecutionThread: KeDelayExecutionThread = SystemExport::new(obfstr::wide!("KeDelayExecutionThread")),

        pub RtlRandomEx: RtlRandomEx = SystemExport::new(obfstr::wide!("RtlRandomEx")),

        pub PsGetCurrentThread: PsGetCurrentThread = SystemExport::new(obfstr::wide!("PsGetCurrentThread")),
        pub PsGetCurrentProcess: PsGetCurrentProcess = SystemExport::new(obfstr::wide!("PsGetCurrentProcess")),
        pub PsGetProcessId: PsGetProcessId = SystemExport::new(obfstr::wide!("PsGetProcessId")),
        pub PsGetProcessPeb: PsGetProcessPeb = SystemExport::new(obfstr::wide!("PsGetProcessPeb")),
        pub PsGetProcessImageFileName: PsGetProcessImageFileName = SystemExport::new(obfstr::wide!("PsGetProcessImageFileName")),
        pub PsLookupProcessByProcessId: PsLookupProcessByProcessId = SystemExport::new(obfstr::wide!("PsLookupProcessByProcessId")),
        pub PsCreateSystemThread: PsCreateSystemThread = SystemExport::new(obfstr::wide!("PsCreateSystemThread")),

        pub MmGetSystemRoutineAddress: MmGetSystemRoutineAddress = SystemExport::new(obfstr::wide!("MmGetSystemRoutineAddress")),
        //pub MmSystemRangeStart: MmSystemRangeStart = SystemExport::new(obfstr::wide!("MmSystemRangeStart")),

        pub IoCreateDeviceSecure: IoCreateDeviceSecure = SystemExport::new(obfstr::wide!("IoCreateDeviceSecure")),

        pub ZwClose: ZwClose = SystemExport::new(obfstr::wide!("ZwClose")),
        
        pub ObfDereferenceObject: ObfDereferenceObject = SystemExport::new(obfstr::wide!("ObfDereferenceObject")),
        pub ObfReferenceObject: ObfReferenceObject = SystemExport::new(obfstr::wide!("ObfReferenceObject")),
        pub ObQueryNameString: ObQueryNameString = SystemExport::new(obfstr::wide!("ObQueryNameString")),
        pub ObReferenceObjectByName: ObReferenceObjectByName = SystemExport::new(obfstr::wide!("ObReferenceObjectByName")),
        pub ObReferenceObjectByHandle: ObReferenceObjectByHandle = SystemExport::new(obfstr::wide!("ObReferenceObjectByHandle")),
    }
}
