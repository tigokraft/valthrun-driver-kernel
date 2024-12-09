#![no_std]

use core::ptr::NonNull;

use obfstr::obfstr;

mod utils;
pub use utils::lookup_image_symbol;

pub fn resolve_system_opt(symbol_name: &str) -> Option<NonNull<()>> {
    let Some(kernelbase) = utils_kernelbase::get() else {
        panic!(
            "{}",
            obfstr!("can not resolve a system import without a kernel base")
        );
    };

    utils::lookup_image_symbol(kernelbase, symbol_name)
        .map(|v| unsafe { NonNull::new_unchecked(v as *mut _) })
}

pub fn resolve_system(_module: Option<&str>, symbol_name: &str) -> NonNull<()> {
    let Some(kernelbase) = utils_kernelbase::get() else {
        panic!(
            "{}",
            obfstr!("can not resolve a system import without a kernel base")
        );
    };

    let Some(result) = utils::lookup_image_symbol(kernelbase, symbol_name) else {
        panic!(
            "{} {} {}",
            obfstr!("system symbol"),
            symbol_name,
            obfstr!("unknown")
        );
    };

    NonNull::new(result as *mut ()).unwrap()
}
