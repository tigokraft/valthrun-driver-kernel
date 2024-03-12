use alloc::{
    format,
    string::ToString,
};
use core::{
    cell::SyncUnsafeCell,
    ptr,
};

use anyhow::Context;
use kapi::{
    Mdl,
    PagePriority,
    IO_READ_ACCESS,
    MCT_CACHED,
};
use kapi_kmodule::{
    KModule,
    KModuleSection,
};
use obfstr::obfstr;
use valthrun_driver_shared::{
    ByteSequencePattern,
    SearchPattern,
    Signature,
    SignatureType,
};
use winapi::{
    ctypes::c_void,
    km::wdm::KPROCESSOR_MODE,
    shared::ntdef::PVOID,
};

/// Undocumented function/struct offsets
/// found by sigscanning
#[allow(non_snake_case)]
pub struct NtOffsets {
    pub PsGetNextProcess: unsafe extern "C" fn(PVOID) -> PVOID,
    pub MmVerifyCallbackFunctionFlags: unsafe extern "C" fn(callback: PVOID, flags: u32) -> bool,

    pub EPROCESS_ThreadListHead: usize,
}

static NT_OFFSETS: SyncUnsafeCell<Option<NtOffsets>> = SyncUnsafeCell::new(None);
pub fn get_nt_offsets() -> &'static NtOffsets {
    let nt_offsets = unsafe { &*NT_OFFSETS.get() };
    nt_offsets.as_ref().unwrap()
}

fn find_mm_verify_callback_function_flags_old(kernel_base: &KModule) -> anyhow::Result<usize> {
    let pattern = ByteSequencePattern::parse(obfstr!(
        "E8 ?? ?? ?? ?? 85 C0 0F 84 ?? ?? ?? ?? 48 8B 4D 00"
    ))
    .with_context(|| {
        obfstr!("Failed to compile MmVerifyCallbackFunctionFlags pattern").to_string()
    })?;

    NtOffsets::locate_function(
        &kernel_base,
        obfstr!("MmVerifyCallbackFunctionFlags"),
        &pattern,
        0x01,
        0x05,
    )
}

fn find_mm_verify_callback_function_flags_new(kernel_base: &KModule) -> anyhow::Result<usize> {
    let pattern = ByteSequencePattern::parse(obfstr!(
        "48 89 5C 24 ? 48 89 6C 24 ? 48 89 74 24 ? 57 48 83 EC 20 8B FA 48 8B F1"
    ))
    .with_context(|| {
        obfstr!("Failed to compile MmVerifyCallbackFunctionFlags pattern").to_string()
    })?;

    kernel_base
        .find_code_sections()?
        .into_iter()
        .filter(KModuleSection::is_data_valid)
        .find_map(|section| {
            if let Some(data) = section.raw_data() {
                if let Some(offset) = pattern.find(data) {
                    Some(offset + section.raw_data_address())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .with_context(|| {
            format!(
                "failed to find {} pattern",
                obfstr!("MmVerifyCallbackFunctionFlags")
            )
        })
}

pub fn initialize_nt_offsets() -> anyhow::Result<()> {
    let ntoskrnl = KModule::find_by_name(obfstr!("ntoskrnl.exe"))?
        .with_context(|| obfstr!("failed to find kernel base").to_string())?;

    let ps_get_next_process = {
        [
            /* Windows 11 */
            Signature::relative_address(
                obfstr!("PsGetNextProcess (Win 11)"),
                obfstr!("E8 ? ? ? ? 48 8B D8 48 85 C0 74 24 F7"),
                0x01,
                0x05,
            ),
            /* Windows 10 19045.4046 */
            Signature::relative_address(
                obfstr!("PsGetNextProcess (19045.4046)"),
                obfstr!("E8 ? ? ? ? 48 8B F8 48 85 C0 74 3B 4C"),
                0x01,
                0x05,
            ),
        ]
        .iter()
        .find_map(|sig| NtOffsets::locate_signature(&ntoskrnl, sig).ok())
        .map(|v| unsafe { core::mem::transmute_copy(&v) })
        .with_context(|| obfstr!("Failed to find PsGetNextProcess").to_string())?
    };

    let mm_verify_callback_function_flags = {
        if let Ok(target) = find_mm_verify_callback_function_flags_new(&ntoskrnl) {
            unsafe { core::mem::transmute_copy::<_, _>(&target) }
        } else {
            log::debug!("{}", obfstr!("Failed to resolve MmVerifyCallbackFunctionFlags by instruction pattern. Try old pattern."));
            if let Ok(target) = find_mm_verify_callback_function_flags_old(&ntoskrnl) {
                unsafe { core::mem::transmute_copy::<_, _>(&target) }
            } else {
                anyhow::bail!(
                    "{}",
                    obfstr!("Failed to resolve MmVerifyCallbackFunctionFlags")
                )
            }
        }
    };

    log::debug!(
        "{}::{} resolved to {:X} ({:X})",
        &ntoskrnl.file_name,
        obfstr!("MmVerifyCallbackFunctionFlags"),
        (mm_verify_callback_function_flags as u64) - ntoskrnl.base_address as u64,
        mm_verify_callback_function_flags as u64
    );
    let eprocess_thread_list_head = {
        [
            /* Windows 11 */
            Signature::offset(
                obfstr!("_EPROCESS.ThreadListHead (Win 11)"),
                obfstr!("4C 8D A9 ? ? ? ? 33 DB"),
                0x03,
            ),
            /* Windows 10 19045.4046 (Actually finds PspGetPreviousProcessThread and PsGetNextProcessThread) */
            Signature::offset(
                obfstr!("_EPROCESS.ThreadListHead (19045.4046)"),
                obfstr!("48 83 EC 20 65 4C 8B 24 25 88 01 00 00 4C 8D B1 ? ? 00 00 45 33 ED"),
                0x10,
            ),
        ]
        .iter()
        .find_map(|sig| NtOffsets::locate_signature(&ntoskrnl, sig).ok())
        .with_context(|| obfstr!("Failed to find _EPROCESS.ThreadListHead").to_string())?
    };

    let offsets = NtOffsets {
        PsGetNextProcess: ps_get_next_process,
        MmVerifyCallbackFunctionFlags: mm_verify_callback_function_flags,

        EPROCESS_ThreadListHead: eprocess_thread_list_head,
    };

    let nt_offsets = unsafe { &mut *NT_OFFSETS.get() };
    *nt_offsets = Some(offsets);

    Ok(())
}

impl NtOffsets {
    pub fn locate_signature(module: &KModule, signature: &Signature) -> anyhow::Result<usize> {
        log::trace!(
            "Resolving '{}' in {}",
            signature.debug_name,
            module.file_name
        );

        let (section, mdl, inst_offset) = module
            .find_code_sections()?
            .into_iter()
            .find_map(|section| {
                if !section.is_data_valid() {
                    return None;
                }

                let mdl = Mdl::allocate(
                    section.raw_data_address() as *mut c_void,
                    section.size_of_raw_data,
                    false,
                    false,
                    ptr::null_mut(),
                )?;

                let mdl = mdl
                    .lock(KPROCESSOR_MODE::KernelMode, IO_READ_ACCESS)
                    .inspect_err(|_| log::warn!("Failed to lock section {}", section.name))
                    .ok()?;

                let mdl = mdl
                    .map(
                        KPROCESSOR_MODE::KernelMode,
                        MCT_CACHED,
                        None,
                        PagePriority::NORMAL,
                    )
                    .inspect_err(|_| log::warn!("Failed to map locked section {}", section.name))
                    .ok()?;

                if let Some(offset) = signature.pattern.find(mdl.as_slice()) {
                    Some((section, mdl, offset))
                } else {
                    None
                }
            })
            .with_context(|| format!("failed to find {} pattern", signature.debug_name))
            .inspect_err(|_| log::trace!("  => not found"))?;

        if matches!(&signature.value_type, SignatureType::Pattern) {
            let address = section.raw_data_address().wrapping_add(inst_offset);
            log::trace!("  => {:X} ({:X})", address, inst_offset);
            return Ok(address);
        }

        let value = unsafe {
            (mdl.address() as *const ())
                .byte_add(inst_offset)
                .byte_add(signature.offset as usize)
                .cast::<u32>()
                .read_unaligned()
        };
        match &signature.value_type {
            SignatureType::Offset => {
                log::trace!("  => {:X} (inst at {:X})", value, inst_offset);
                Ok(value as usize)
            }
            SignatureType::RelativeAddress { inst_length } => {
                let value = section
                    .raw_data_address()
                    .wrapping_add(inst_offset)
                    .wrapping_add(*inst_length)
                    .wrapping_add_signed(value as isize) as usize;
                log::trace!(
                    "  => {:X} ({:X})",
                    value,
                    value - section.raw_data_address()
                );
                Ok(value)
            }
            SignatureType::Pattern => unreachable!(),
        }
    }

    pub fn locate_function<T>(
        module: &KModule,
        name: &str,
        pattern: &dyn SearchPattern,
        offset_rel_address: isize,
        instruction_length: usize,
    ) -> anyhow::Result<T>
    where
        T: Sized,
    {
        let pattern_match = module
            .find_code_sections()?
            .into_iter()
            .find_map(|section| {
                // if let Some(memory) = LockedVirtMem::create(section.raw_data_address() as u64, section.size_of_raw_data, winapi::km::wdm::KPROCESSOR_MODE::UserMode, IO_READ_ACCESS, MCT_CACHED) {
                //     if let Some(offset) = pattern.find(memory.memory()) {
                //         Some(offset + section.raw_data_address())
                //     } else {
                //         None
                //     }
                // } else {
                //     log::warn!(
                //         "Skipping {}::{} as section data could not be locked",
                //         module.file_name,
                //         section.name
                //     );
                //     None
                // }
                if let Some(data) = section.raw_data() {
                    if let Some(offset) = pattern.find(data) {
                        Some(offset + section.raw_data_address())
                    } else {
                        None
                    }
                } else {
                    log::warn!(
                        "Skipping {}::{} as section data is not valid / paged out",
                        module.file_name,
                        section.name
                    );
                    None
                }
            })
            .with_context(|| format!("failed to find {} pattern", name))?;

        let offset = unsafe {
            (pattern_match as *const ())
                .byte_offset(offset_rel_address)
                .cast::<i32>()
                .read_unaligned()
        };

        let target = pattern_match
            .wrapping_add_signed(offset as isize)
            .wrapping_add(instruction_length);

        log::debug!(
            "{}::{} located at {:X} ({:X})",
            module.file_name,
            name,
            target - module.base_address,
            target
        );
        unsafe { Ok(core::mem::transmute_copy::<_, T>(&target)) }
    }

    pub fn locate_offset(
        module: &KModule,
        name: &str,
        pattern: &dyn SearchPattern,
        inst_offset: isize,
    ) -> anyhow::Result<usize> {
        let pattern_match = module
            .find_code_sections()?
            .into_iter()
            .find_map(|section| {
                if let Some(data) = section.raw_data() {
                    if let Some(offset) = pattern.find(data) {
                        Some(offset + section.raw_data_address())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .with_context(|| format!("failed to find {} pattern", name))?;

        let offset = unsafe {
            (pattern_match as *const ())
                .byte_offset(inst_offset)
                .cast::<u32>()
                .read_unaligned()
        };

        log::debug!("{}::{} resolved to {:X}", module.file_name, name, offset);
        Ok(offset as usize)
    }
}
