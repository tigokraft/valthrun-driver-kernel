use core::ffi::CStr;

use pelite::pe::{
    exports::{
        Export,
        GetProcAddress,
    },
    PeView,
};
use winapi::um::winnt::{
    IMAGE_DIRECTORY_ENTRY_EXPORT,
    IMAGE_DOS_HEADER,
    IMAGE_EXPORT_DIRECTORY,
    IMAGE_NT_HEADERS,
};

pub fn lookup_image_symbol(image_base: u64, symbol_name: &str) -> Option<u64> {
    let image = unsafe { PeView::module(image_base as *const u8) };
    image
        .get_export(symbol_name)
        .ok()
        .map(Export::symbol)
        .flatten()
        .map(|rva| image_base + rva as u64)
}
