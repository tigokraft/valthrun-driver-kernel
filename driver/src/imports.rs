#![allow(dead_code)]

use kdef::{
    OBJECT_NAME_INFORMATION,
    POBJECT_TYPE,
    _KAPC_STATE,
    _OB_CALLBACK_REGISTRATION,
    _PEB,
};
use utils_imports::{
    dynamic_import_table,
    provider::SystemExport,
};
use winapi::{
    km::wdm::{
        DEVICE_OBJECT,
        DEVICE_TYPE,
        DRIVER_OBJECT,
        IO_PRIORITY::KPRIORITY_BOOST,
        KPROCESSOR_MODE,
        PDEVICE_OBJECT,
        PEPROCESS,
        PETHREAD,
        PIRP,
    },
    shared::{
        guiddef::LPCGUID,
        ntdef::{
            BOOLEAN,
            CCHAR,
            HANDLE,
            KIRQL,
            NTSTATUS,
            PCUNICODE_STRING,
            PCVOID,
            PHANDLE,
            POBJECT_ATTRIBUTES,
            PVOID,
            UNICODE_STRING,
        },
    },
    um::winnt::{
        ACCESS_MASK,
        PIMAGE_NT_HEADERS,
    },
};

use crate::wsk::sys::{
    IN6_ADDR,
    IN_ADDR,
    PMDL,
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
type RtlImageNtHeader = unsafe extern "C" fn(ModuleAddress: PVOID) -> PIMAGE_NT_HEADERS;

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
type KeDelayExecutionThread = unsafe extern "C" fn(
    WaitMode: KPROCESSOR_MODE,
    Alertable: bool,
    Interval: *const u64,
) -> NTSTATUS;

type MmGetSystemRoutineAddress =
    unsafe extern "C" fn(system_routine_name: *const UNICODE_STRING) -> PVOID;

type ZwQuerySystemInformation = unsafe extern "system" fn(
    SystemInformationClass: u32,
    SystemInformation: *mut (),
    SystemInformationLength: u32,
    ReturnLength: *mut u32,
) -> NTSTATUS;
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
type IoAllocateIrp = unsafe extern "system" fn(StackSize: CCHAR, ChargeQuota: bool) -> PIRP;
type IoCancelIrp = unsafe extern "system" fn(Irp: PIRP);
type IoCompleteRequest = unsafe extern "system" fn(Irp: PIRP, PriorityBoost: KPRIORITY_BOOST);
type IoDeleteDevice = unsafe extern "system" fn(DeviceObject: *mut DEVICE_OBJECT) -> NTSTATUS;
type IoAllocateMdl = unsafe extern "system" fn(
    VirtualAddress: PVOID,
    Length: u32,
    SecondaryBuffer: bool,
    ChargeQuota: bool,
    Irp: PIRP,
) -> PMDL;
type IoFreeMdl = unsafe extern "system" fn(MemoryDescriptorList: PMDL);
type IoFreeIrp = unsafe extern "system" fn(Irp: PIRP);
type IoRegisterShutdownNotification = unsafe extern "system" fn(DeviceObject: PDEVICE_OBJECT) -> NTSTATUS;
type IoUnregisterShutdownNotification = unsafe extern "system" fn(DeviceObject: PDEVICE_OBJECT);

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
type ObRegisterCallbacks = unsafe extern "system" fn(
    CallbackRegistration: *const _OB_CALLBACK_REGISTRATION,
    RegistrationHandle: *mut PVOID,
) -> NTSTATUS;
type ObUnRegisterCallbacks = unsafe extern "system" fn(RegistrationHandle: PVOID);

type MmUnlockPages = unsafe extern "system" fn(MemoryDescriptorList: PMDL);
type MmMapLockedPagesSpecifyCache = unsafe extern "system" fn(
    MemoryDescriptorList: PMDL,
    AccessMode: KPROCESSOR_MODE,
    CacheType: u32,
    RequestedAddress: PVOID,
    BugCheckOnFailure: u32,
    Priority: u32,
) -> PVOID;
type MmIsAddressValid = unsafe extern "system" fn(Address: PVOID) -> bool;
type MmCopyVirtualMemory = unsafe extern "system" fn(
    FromProcess: PEPROCESS,
    FromAddress: PCVOID,
    ToProcess: PEPROCESS,
    ToAddress: PVOID,
    BufferSize: usize,
    PreviousMode: KPROCESSOR_MODE,
    NumberOfBytesCopied: *mut usize,
) -> NTSTATUS;

type ExGetSystemFirmwareTable = unsafe extern "system" fn(
    FirmwareTableProviderSignature: u32,
    FirmwareTableID: u32,
    FirmwareTableBuffer: PVOID,
    BufferLength: u32,
    ReturnLength: *mut u32,
) -> NTSTATUS;

dynamic_import_table! {
    pub imports GLOBAL_IMPORTS {
        pub RtlIpv4AddressToStringExA: RtlIpv4AddressToStringExA = SystemExport::new(obfstr!("RtlIpv4AddressToStringExA")),
        pub RtlIpv6AddressToStringExA: RtlIpv6AddressToStringExA = SystemExport::new(obfstr!("RtlIpv6AddressToStringExA")),
        pub RtlImageNtHeader: RtlImageNtHeader = SystemExport::new(obfstr!("RtlImageNtHeader")),

        pub KeQuerySystemTimePrecise: KeQuerySystemTimePrecise = SystemExport::new(obfstr!("KeQuerySystemTimePrecise")),
        pub KeQueryTimeIncrement: KeQueryTimeIncrement = SystemExport::new(obfstr!("KeQueryTimeIncrement")),
        pub KeStackAttachProcess: KeStackAttachProcess = SystemExport::new(obfstr!("KeStackAttachProcess")),
        pub KeUnstackDetachProcess: KeUnstackDetachProcess = SystemExport::new(obfstr!("KeUnstackDetachProcess")),
        pub KeDelayExecutionThread: KeDelayExecutionThread = SystemExport::new(obfstr!("KeDelayExecutionThread")),

        pub RtlRandomEx: RtlRandomEx = SystemExport::new(obfstr!("RtlRandomEx")),

        pub PsGetCurrentThread: PsGetCurrentThread = SystemExport::new(obfstr!("PsGetCurrentThread")),
        pub PsGetCurrentProcess: PsGetCurrentProcess = SystemExport::new(obfstr!("PsGetCurrentProcess")),
        pub PsGetProcessId: PsGetProcessId = SystemExport::new(obfstr!("PsGetProcessId")),
        pub PsGetProcessPeb: PsGetProcessPeb = SystemExport::new(obfstr!("PsGetProcessPeb")),
        pub PsGetProcessImageFileName: PsGetProcessImageFileName = SystemExport::new(obfstr!("PsGetProcessImageFileName")),
        pub PsLookupProcessByProcessId: PsLookupProcessByProcessId = SystemExport::new(obfstr!("PsLookupProcessByProcessId")),
        pub PsCreateSystemThread: PsCreateSystemThread = SystemExport::new(obfstr!("PsCreateSystemThread")),

        pub MmGetSystemRoutineAddress: MmGetSystemRoutineAddress = SystemExport::new(obfstr!("MmGetSystemRoutineAddress")),
        //pub MmSystemRangeStart: MmSystemRangeStart = SystemExport::new(obfstr!("MmSystemRangeStart")),

        pub IoCreateDeviceSecure: IoCreateDeviceSecure = SystemExport::new(obfstr!("IoCreateDeviceSecure")),
        pub IoDeleteDevice: IoDeleteDevice = SystemExport::new(obfstr!("IoDeleteDevice")),
        pub IoAllocateIrp: IoAllocateIrp = SystemExport::new(obfstr!("IoAllocateIrp")),
        pub IoCompleteRequest: IoCompleteRequest = SystemExport::new(obfstr!("IoCompleteRequest")),
        pub IoCancelIrp: IoCancelIrp = SystemExport::new(obfstr!("IoCancelIrp")),
        pub IoFreeIrp: IoFreeIrp = SystemExport::new(obfstr!("IoFreeIrp")),
        pub IoAllocateMdl: IoAllocateMdl = SystemExport::new(obfstr!("IoAllocateMdl")),
        pub IoFreeMdl: IoFreeMdl = SystemExport::new(obfstr!("IoFreeMdl")),
        pub IoRegisterShutdownNotification: IoRegisterShutdownNotification = SystemExport::new(obfstr!("IoRegisterShutdownNotification")),
        pub IoUnregisterShutdownNotification: IoUnregisterShutdownNotification = SystemExport::new(obfstr!("IoUnregisterShutdownNotification")),

        pub ZwQuerySystemInformation: ZwQuerySystemInformation = SystemExport::new(obfstr!("ZwQuerySystemInformation")),
        pub ZwClose: ZwClose = SystemExport::new(obfstr!("ZwClose")),

        pub ObfDereferenceObject: ObfDereferenceObject = SystemExport::new(obfstr!("ObfDereferenceObject")),
        pub ObfReferenceObject: ObfReferenceObject = SystemExport::new(obfstr!("ObfReferenceObject")),
        pub ObQueryNameString: ObQueryNameString = SystemExport::new(obfstr!("ObQueryNameString")),
        pub ObReferenceObjectByName: ObReferenceObjectByName = SystemExport::new(obfstr!("ObReferenceObjectByName")),
        pub ObReferenceObjectByHandle: ObReferenceObjectByHandle = SystemExport::new(obfstr!("ObReferenceObjectByHandle")),
        pub ObRegisterCallbacks: ObRegisterCallbacks = SystemExport::new(obfstr!("ObRegisterCallbacks")),
        pub ObUnRegisterCallbacks: ObUnRegisterCallbacks = SystemExport::new(obfstr!("ObUnRegisterCallbacks")),

        pub MmUnlockPages: MmUnlockPages = SystemExport::new(obfstr!("MmUnlockPages")),
        pub MmMapLockedPagesSpecifyCache: MmMapLockedPagesSpecifyCache = SystemExport::new(obfstr!("MmMapLockedPagesSpecifyCache")),
        pub MmIsAddressValid: MmIsAddressValid = SystemExport::new(obfstr!("MmIsAddressValid")),
        pub MmCopyVirtualMemory: MmCopyVirtualMemory = SystemExport::new(obfstr!("MmCopyVirtualMemory")),

        pub ExGetSystemFirmwareTable: ExGetSystemFirmwareTable = SystemExport::new(obfstr!("ExGetSystemFirmwareTable")),
    }
}

type IoCreateDriver =
    unsafe extern "system" fn(name: *const UNICODE_STRING, entry: *const ()) -> NTSTATUS;
type KeGetCurrentIrql = unsafe extern "system" fn() -> KIRQL;
dynamic_import_table! {
    pub imports LL_GLOBAL_IMPORTS {
        pub IoCreateDriver: IoCreateDriver = SystemExport::new(obfstr!("IoCreateDriver")),
        pub KeGetCurrentIrql: KeGetCurrentIrql = SystemExport::new(obfstr!("KeGetCurrentIrql")),
        pub MmSystemRangeStart: *const u64 = SystemExport::new(obfstr!("MmSystemRangeStart")),
    }
}
