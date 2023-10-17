use utils_imports::{dynamic_import_table, provider::SystemExport};
use winapi::{shared::ntdef::{PVOID, NTSTATUS}, um::winnt::PIMAGE_NT_HEADERS};

type RtlImageNtHeader = unsafe extern "C" fn(ModuleAddress: PVOID) -> PIMAGE_NT_HEADERS;

type ZwQuerySystemInformation = unsafe extern "system" fn(
    SystemInformationClass: u32,
    SystemInformation: *mut (),
    SystemInformationLength: u32,
    ReturnLength: *mut u32,
) -> NTSTATUS;

type MmIsAddressValid = unsafe extern "system" fn(Address: PVOID) -> bool;

dynamic_import_table! {
    pub imports GLOBAL_IMPORTS {
        pub ZwQuerySystemInformation: ZwQuerySystemInformation = SystemExport::new(obfstr!("ZwQuerySystemInformation")),
        pub RtlImageNtHeader: RtlImageNtHeader = SystemExport::new(obfstr!("RtlImageNtHeader")),
        pub MmIsAddressValid: MmIsAddressValid = SystemExport::new(obfstr!("MmIsAddressValid")),
    }
}