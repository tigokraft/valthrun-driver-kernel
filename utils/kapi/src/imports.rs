#![allow(non_snake_case)]
#![allow(unused)]

use kdef::{
    OBJECT_NAME_INFORMATION,
    POBJECT_TYPE,
    _KAPC_STATE,
    _MDL,
    _OB_CALLBACK_REGISTRATION,
    _PEB,
};
use lazy_link::lazy_link;
use winapi::{
    km::{
        ndis::PMDL,
        wdm::{
            DEVICE_OBJECT,
            DEVICE_TYPE,
            DRIVER_OBJECT,
            IO_PRIORITY::KPRIORITY_BOOST,
            KPROCESSOR_MODE,
            PDEVICE_OBJECT,
            PEPROCESS,
            PETHREAD,
            PIRP,
            POOL_TYPE,
        },
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

pub const BCRYPT_RNG_USE_ENTROPY_IN_BUFFER: u32 = 0x00000001;
pub const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x00000002;

#[lazy_link(resolver = "kapi_kmodule::resolve_import")]
extern "C" {
    pub fn RtlImageNtHeader(ModuleAddress: PVOID) -> PIMAGE_NT_HEADERS;

    pub fn KeQuerySystemTimePrecise(CurrentTime: *mut u64) -> ();
    pub fn KeQueryTimeIncrement() -> u32;
    pub fn KeStackAttachProcess(process: PEPROCESS, apc_state: *mut _KAPC_STATE) -> ();
    pub fn KeUnstackDetachProcess(apc_state: *mut _KAPC_STATE) -> ();
    pub fn KeWaitForSingleObject(
        Object: PVOID,
        WaitReason: u32,
        WaitMode: KPROCESSOR_MODE,
        Alertable: bool,
        Timeout: *const i64,
    ) -> NTSTATUS;
    pub fn KeDelayExecutionThread(
        WaitMode: KPROCESSOR_MODE,
        Alertable: bool,
        Interval: *const i64,
    ) -> NTSTATUS;

    pub fn RtlRandomEx(Seed: *mut u32) -> u32;

    pub fn PsGetCurrentThread() -> PETHREAD;
    pub fn PsGetCurrentProcess() -> PEPROCESS;
    pub fn PsGetProcessId(process: PEPROCESS) -> i32;
    pub fn PsGetProcessPeb(process: PEPROCESS) -> *const _PEB;
    pub fn PsLookupProcessByProcessId(process_id: i32, process: *mut PEPROCESS) -> NTSTATUS;
    pub fn PsGetProcessImageFileName(Process: PEPROCESS) -> *const i8;
    pub fn PsCreateSystemThread(
        ThreadHandle: PHANDLE,
        DesiredAccess: u32,
        ObjectAttributes: POBJECT_ATTRIBUTES,
        ProcessHandle: HANDLE,
        ClientId: *mut u32,
        StartRoutine: extern "C" fn(PVOID) -> (),
        StartContext: PVOID,
    ) -> NTSTATUS;

    pub fn MmGetSystemRoutineAddress(system_routine_name: *const UNICODE_STRING) -> PVOID;
    //pub MmSystemRangeStart: MmSystemRangeStart = SystemExport::new(obfstr!("MmSystemRangeStart")),

    pub fn IoCreateDeviceSecure(
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
    pub fn IoDeleteDevice(DeviceObject: *mut DEVICE_OBJECT) -> NTSTATUS;

    pub fn IoAllocateIrp(StackSize: CCHAR, ChargeQuota: bool) -> PIRP;
    pub fn IoCancelIrp(Irp: PIRP);
    pub fn IoCompleteRequest(Irp: PIRP, PriorityBoost: KPRIORITY_BOOST);
    pub fn IoFreeIrp(Irp: PIRP);

    pub fn IoAllocateMdl(
        VirtualAddress: PVOID,
        Length: u32,
        SecondaryBuffer: bool,
        ChargeQuota: bool,
        Irp: PIRP,
    ) -> *mut _MDL;
    pub fn IoFreeMdl(MemoryDescriptorList: *mut _MDL);

    pub fn ZwQuerySystemInformation(
        SystemInformationClass: u32,
        SystemInformation: *mut (),
        SystemInformationLength: u32,
        ReturnLength: *mut u32,
    ) -> NTSTATUS;
    pub fn ZwClose(Handle: HANDLE) -> NTSTATUS;

    pub fn ObfReferenceObject(object: PVOID);
    pub fn ObfDereferenceObject(object: PVOID);
    pub fn ObQueryNameString(
        Object: PVOID,
        ObjectNameInfo: *mut OBJECT_NAME_INFORMATION,
        Length: u32,
        ReturnLength: &mut u32,
    ) -> NTSTATUS;
    pub fn ObReferenceObjectByName(
        ObjectName: PCUNICODE_STRING,
        Attributes: u32,
        AccessState: *mut (),
        DesiredAccess: ACCESS_MASK,
        ObjectType: POBJECT_TYPE,
        AccessMode: KPROCESSOR_MODE,
        ParseContext: PVOID,
        Object: PVOID,
    ) -> NTSTATUS;
    pub fn ObReferenceObjectByHandle(
        Handle: HANDLE,
        DesiredAccess: ACCESS_MASK,
        ObjectType: POBJECT_TYPE,
        AccessMode: KPROCESSOR_MODE,
        Object: PVOID,
        HandleInformation: PVOID,
    ) -> NTSTATUS;

    pub fn MmUnlockPages(MemoryDescriptorList: *mut _MDL);
    pub fn MmMapLockedPagesSpecifyCache(
        MemoryDescriptorList: *mut _MDL,
        AccessMode: KPROCESSOR_MODE,
        CacheType: u32,
        RequestedAddress: PVOID,
        BugCheckOnFailure: u32,
        Priority: u32,
    ) -> PVOID;
    pub fn MmUnmapLockedPages(BaseAddress: PVOID, MemoryDescriptorList: *mut _MDL) -> ();
    pub fn MmIsAddressValid(Address: PVOID) -> bool;
}
