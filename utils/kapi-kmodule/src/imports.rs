#![allow(non_snake_case)]

use core::ptr::NonNull;

use lazy_link::lazy_link;
use obfstr::obfstr;
use winapi::{
    shared::ntdef::{
        NTSTATUS,
        PVOID,
    },
    um::winnt::PIMAGE_NT_HEADERS,
};

use crate::KModule;

#[lazy_link(resolver = "utils_imports::resolve_system")]
extern "system" {
    pub fn RtlImageNtHeader(ModuleAddress: PVOID) -> PIMAGE_NT_HEADERS;
    pub fn ZwQuerySystemInformation(
        SystemInformationClass: u32,
        SystemInformation: *mut (),
        SystemInformationLength: u32,
        ReturnLength: *mut u32,
    ) -> NTSTATUS;

    pub fn MmIsAddressValid(Address: PVOID) -> bool;
}

pub fn resolve_import(module: Option<&str>, symbol_name: &str) -> NonNull<()> {
    let Some(module) = module else {
        let result = utils_imports::resolve_system(module, symbol_name);
        // log::trace!(
        //     "Resolved kernel symbol {} to 0x{:X}",
        //     symbol_name,
        //     result.as_ptr() as u64
        // );
        return result;
    };

    let net_module = KModule::find_by_name(module).unwrap().unwrap();
    let Some(result) =
        utils_imports::lookup_image_symbol(net_module.base_address as u64, symbol_name)
    else {
        panic!(
            "{} {}::{} {}",
            obfstr!("symbol"),
            module,
            symbol_name,
            obfstr!("unknown")
        );
    };

    // log::trace!(
    //     "Resolved symbol {}::{} to 0x{:X}",
    //     module,
    //     symbol_name,
    //     result
    // );
    NonNull::new(result as *mut ()).unwrap()
}
