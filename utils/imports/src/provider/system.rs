use core::{
    arch::asm,
    fmt::Debug,
    sync::atomic::{
        AtomicU64,
        Ordering,
    },
};

use windows_sys::Win32::System::{
    Diagnostics::Debug::IMAGE_NT_HEADERS64,
    SystemServices::IMAGE_DOS_HEADER,
};

use crate::{
    utils,
    DynamicImport,
    DynamicImportError,
    ImportResult,
};

/// Provider for ntos imports.
/// This provider does not allocate any heap memory.
///
/// Note:
/// You must call `SystemExport::initialize` before resolving any system exports.
#[derive(Debug)]
pub struct SystemExport<'a> {
    function: &'a str,
}

impl<'a> SystemExport<'a> {
    pub fn new(function: &'a str) -> Self {
        Self { function }
    }

    /// Initialize the system export provider.
    ///
    /// Attention:
    /// If the ntoskrnl can not be located this function will cause a BSOD!
    pub fn initialize(ntoskrnl_base: Option<u64>) {
        let ntoskrnl_base = ntoskrnl_base.unwrap_or_else(find_ntoskrnl_image);
        WINDOWS_KERNEL_BASE.store(ntoskrnl_base, Ordering::Relaxed);
    }
}

impl<'a, T> DynamicImport<T> for SystemExport<'a> {
    fn resolve(self) -> ImportResult<T> {
        let ntoskrnl_base = WINDOWS_KERNEL_BASE.load(Ordering::Relaxed);
        if ntoskrnl_base == 0 {
            /* ntoskrnl has not yet been initialized */
            return Err(DynamicImportError::ProviderNotInitialized);
        }

        utils::resolve_symbol_from_pimage(ntoskrnl_base, self.function)
            .map(|value| unsafe { core::mem::transmute_copy(&value) })
            .ok_or(DynamicImportError::SymbolUnknown)
    }
}

static WINDOWS_KERNEL_BASE: AtomicU64 = AtomicU64::new(0);

/// Find the NT kernel base address.
///   
/// Attention:
/// This **will** cause a BSOD when the kernel has not been yet loaded.
fn find_ntoskrnl_image() -> u64 {
    let idt_table: *const ();
    unsafe { asm!("mov {}, gs:38h", out(reg) idt_table) };

    /* Read the first IDT entry in IdtBase */
    let mut current_ntoskrnl_page =
        unsafe { idt_table.byte_add(0x04).cast::<u64>().read_unaligned() };
    current_ntoskrnl_page &= !0xFFF;

    loop {
        /*
         * Search the next page for the PE header.
         *
         * Note:
         * The IDT will never be at the first 4kb page of the DOS header therefore it's okey to always subtract 4k
         */
        current_ntoskrnl_page -= 0x1000;

        let dos_header = unsafe { &*(current_ntoskrnl_page as *const IMAGE_DOS_HEADER) };
        if dos_header.e_magic != 0x5A4D {
            /* DOS header does not matches */
            continue;
        }

        let nt_headers = unsafe {
            &*((current_ntoskrnl_page + dos_header.e_lfanew as u64) as *const IMAGE_NT_HEADERS64)
        };
        if nt_headers.Signature != 0x00004550 {
            /* NT header does not match */
            continue;
        }

        if nt_headers.FileHeader.NumberOfSections < 0x20 {
            /* we found a valid PE, but it's not ntoskrnl as number of sections should be quite high */
            continue;
        }

        /* This PE image seems like the ntoskrnl */
        return current_ntoskrnl_page;
    }
}
