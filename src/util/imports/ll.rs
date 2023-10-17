use core::{cell::SyncUnsafeCell, arch::asm, ffi::CStr};

use winapi::um::winnt::{PIMAGE_DOS_HEADER, PIMAGE_NT_HEADERS, IMAGE_DIRECTORY_ENTRY_EXPORT, PIMAGE_EXPORT_DIRECTORY};

pub static KERNEL_BASE: SyncUnsafeCell<u64> = SyncUnsafeCell::new(0);

fn find_kernel_base() -> u64 {
    let idt_table: *const ();
    unsafe { asm!("mov {}, gs:38h", out(reg) idt_table) };

    /* Read the first IDT entry in IdtBase */
    let mut current_ntoskrnl_page = unsafe { idt_table.byte_add(0x04).cast::<u64>().read_unaligned() };
    current_ntoskrnl_page &= !0x3FFF;

    loop {
        let header = unsafe { *(current_ntoskrnl_page as *const u16) };
        if header == 0x5A4D {
            break;
        }

        /* search the next page for the PE header */
        current_ntoskrnl_page -= 0x4000;
    }

    current_ntoskrnl_page
}

/*
 * Initialize low level import system.
 * This function will always succeed or end up in a BSOD. 
 */
pub fn init_import_ll() {
    unsafe { *KERNEL_BASE.get() = find_kernel_base() }
}

pub fn lookup_export(base_address: u64, target: &str) -> Option<u64> {
    let dos_header = unsafe { &*(base_address as PIMAGE_DOS_HEADER) };

    let nt_headers = unsafe { &*((base_address + dos_header.e_lfanew as u64) as PIMAGE_NT_HEADERS) };
    let export_table = &nt_headers.OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXPORT as usize];

    let export_directory = unsafe { &*((base_address + export_table.VirtualAddress as u64) as PIMAGE_EXPORT_DIRECTORY) };
    let name_table = unsafe { 
        core::slice::from_raw_parts(
            (base_address + export_directory.AddressOfNames as u64) as *const u32, 
            export_directory.NumberOfNames as usize
        ) 
    };
    let name_ordinals = unsafe {
        core::slice::from_raw_parts(
            (base_address + export_directory.AddressOfNameOrdinals as u64) as *const u16, 
            export_directory.NumberOfNames as usize
        ) 
    };
    let export_functions = unsafe {
        core::slice::from_raw_parts(
            (base_address + export_directory.AddressOfFunctions as u64) as *const u32, 
            export_directory.NumberOfFunctions as usize
        ) 
    };

    // TODO: Implement binary search instead of looping trough every entry
    for (name_index, name_rva) in name_table.iter().enumerate() {
        let name = unsafe { CStr::from_ptr((base_address + *name_rva as u64) as *const i8) };
        let name = match name.to_str() {
            Ok(name) => name,
            Err(_) => continue,
        };

        if name != target {
            continue;
        }

        /* name_ordinals has equal size as name_table */
        let ordinal = name_ordinals[name_index];
        let function_offset = match export_functions.get(ordinal as usize) {
            Some(function) => function,
            None => {
                /* 
                 * Invalid function entry.
                 * Should not happen but we can't do proper error handling yet. 
                 * Therefore we just dismiss this error.
                 */
                continue;
            }
        };

        return Some(base_address + *function_offset as u64);
    }

    None
}

pub fn lookup_system_export(target: &str) -> Option<u64> {
    let kernel_base = unsafe { *KERNEL_BASE.get() };
    if kernel_base > 0 {
        lookup_export(kernel_base, target)
    } else {
        None
    }
}