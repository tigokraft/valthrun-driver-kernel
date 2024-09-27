use core::ffi::CStr;

use winapi::um::winnt::{
    IMAGE_DIRECTORY_ENTRY_EXPORT,
    IMAGE_DOS_HEADER,
    IMAGE_EXPORT_DIRECTORY,
    IMAGE_NT_HEADERS,
};

pub fn lookup_image_symbol(image_base: u64, symbol_name: &str) -> Option<u64> {
    let dos_header = unsafe { &*(image_base as *const IMAGE_DOS_HEADER) };

    let nt_headers =
        unsafe { &*((image_base + dos_header.e_lfanew as u64) as *const IMAGE_NT_HEADERS) };
    let export_table =
        &nt_headers.OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXPORT as usize];

    let export_directory = unsafe {
        &*((image_base + export_table.VirtualAddress as u64) as *const IMAGE_EXPORT_DIRECTORY)
    };
    let name_table = unsafe {
        core::slice::from_raw_parts(
            (image_base + export_directory.AddressOfNames as u64) as *const u32,
            export_directory.NumberOfNames as usize,
        )
    };
    let name_ordinals = unsafe {
        core::slice::from_raw_parts(
            (image_base + export_directory.AddressOfNameOrdinals as u64) as *const u16,
            export_directory.NumberOfNames as usize,
        )
    };
    let export_functions = unsafe {
        core::slice::from_raw_parts(
            (image_base + export_directory.AddressOfFunctions as u64) as *const u32,
            export_directory.NumberOfFunctions as usize,
        )
    };

    // TODO: Implement binary search instead of looping trough every entry
    for (name_index, name_rva) in name_table.iter().enumerate() {
        let name = unsafe { CStr::from_ptr((image_base + *name_rva as u64) as *const i8) };
        let name = match name.to_str() {
            Ok(name) => name,
            Err(_) => continue,
        };

        if name != symbol_name {
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

        return Some(image_base + *function_offset as u64);
    }

    None
}
