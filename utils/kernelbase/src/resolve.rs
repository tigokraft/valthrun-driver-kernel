use core::{
    arch::asm,
    sync::atomic::{
        AtomicU64,
        Ordering,
    },
};

use winapi::um::winnt::{
    IMAGE_DOS_HEADER,
    IMAGE_NT_HEADERS,
};

use crate::{
    def::KIDTEntry64,
    utils::search_binary_pattern,
};

/// Find the NT kernel base address.
///
/// This is quite tricky as recent Windows versions contain a gap between the .text / KVASCODE section
/// and the NT header thus a simple backwards search would cause a BSOD.
///
/// Solution for the finding the kernel base:
/// 1. Find a valid entry within the KVASCODE section (or .text section) by
///    by using the first entry from the IDT which will always be KiDivideErrorFaultShadow.
/// 2. Resolve the actual function located within a .text section from the shadow function
/// 3. Locate a reference to the .rdata section which is currently right before the NT header.
/// 4. Backwards search towards the NT header
///   
/// Attention:
/// This **will** cause a BSOD when the kernel has not been yet loaded.
fn find_ntoskrnl_image() -> u64 {
    let idt_table: *const KIDTEntry64;
    unsafe { asm!("mov {}, gs:38h", out(reg) idt_table) };

    /*
     * Read the first idt entry (KiDivideErrorFaultShadow) which will be within the KVASCODE section.
     * For older kernels this entry will be somewhere within the .text section.
     */
    let idt_entry = unsafe { *idt_table };
    let idt_handler = idt_entry.offset_low as u64 |
        ((idt_entry.offset_middle as u64) << 16) |
        ((idt_entry.offset_high as u64) << 32);

    let text_page = {
        // KVASCODE:000000000099316E 0F AE E8                                      lfence
        // KVASCODE:0000000000993171 E9 8A 50 A1 FF                                jmp     KiDivideErrorFault
        let pattern = [0x0F, 0xAE, 0xE8, 0xE9];

        let address = search_binary_pattern(idt_handler, &pattern, 0x00, 0x01) + 0x03;
        let offset = unsafe { *((address + 0x01) as *const i32) } as i64;
        (address + 0x05) as i64 + offset
    } as u64;

    /*
     * Search for a reference to the .rdata section within the PAGE page.
     * Note:
     * We assume, that the PAGE page will be followed after the .text page which is generally the case.
     */
    let rdata_page = {
        // PAGE:0000000000678AC7 48 8D 05 32 26 99 FF                          lea     rax, aAllowdevelopme ; "AllowDevelopmentWithoutDevLicense"
        // PAGE:0000000000678ACE 49 C7 43 E8 42 00 44 00                       mov     qword ptr [r11-18h], 440042h
        // PAGE:0000000000678AD6 49 8D 53 08                                   lea     rdx, [r11+8]
        // PAGE:0000000000678ADA 49 89 43 F0                                   mov     [r11-10h], rax
        const DUMMY: u8 = 0xAA;
        let pattern = [
            0x48, 0x8D, 0x05, DUMMY, DUMMY, DUMMY, DUMMY, 0x49, 0xC7, 0x43, DUMMY, DUMMY, DUMMY,
            DUMMY, DUMMY, 0x49,
        ];

        /* backwards scan for code pattern to skip the gap between .text and .rdata */
        let address = search_binary_pattern(text_page, &pattern, DUMMY, 0x01);
        let offset = unsafe { *((address + 0x03) as *const i32) } as i64;
        (address + 0x07) as i64 + offset
    } as u64;

    /*
     * From the .rdata section search upwards untill we found the
     * NT header.
     */
    let mut kernel_base = (rdata_page & !0xFFF) + 0x1000;
    loop {
        kernel_base -= 0x1000;

        let dos_header = unsafe { &*(kernel_base as *const IMAGE_DOS_HEADER) };
        if dos_header.e_magic != 0x5A4D {
            /* DOS header does not matches */
            continue;
        }

        let nt_headers =
            unsafe { &*((kernel_base + dos_header.e_lfanew as u64) as *const IMAGE_NT_HEADERS) };
        if nt_headers.Signature != 0x00004550 {
            /* NT header does not match */
            continue;
        }

        if nt_headers.FileHeader.NumberOfSections < 0x18 {
            /* we found a valid PE, but it's not ntoskrnl as number of sections should be quite high */
            continue;
        }

        /* This PE image seems like the ntoskrnl */
        return kernel_base;
    }
}

static WINDOWS_KERNEL_BASE: AtomicU64 = AtomicU64::new(0);

/// Initialize the system export provider.
///
/// Attention:
/// If the ntoskrnl can not be located this function will cause a BSOD!
pub fn initialize(ntoskrnl_base: Option<u64>) {
    let ntoskrnl_base = ntoskrnl_base.unwrap_or_else(find_ntoskrnl_image);
    WINDOWS_KERNEL_BASE.store(ntoskrnl_base, Ordering::Relaxed);
}

pub fn get() -> Option<u64> {
    let value = WINDOWS_KERNEL_BASE.load(Ordering::Relaxed);
    if value > 0 {
        Some(value)
    } else {
        None
    }
}
