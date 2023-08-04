use core::cell::SyncUnsafeCell;

use alloc::{string::ToString, format};
use anyhow::Context;
use obfstr::obfstr;
use valthrun_driver_shared::{ByteSequencePattern, SearchPattern};
use winapi::shared::ntdef::PVOID;

use crate::kapi::KModule;

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

pub fn initialize_nt_offsets() -> anyhow::Result<()> {
    let kernel_base = KModule::find_by_name(obfstr!("ntoskrnl.exe"))?
        .with_context(|| obfstr!("failed to find kernel base").to_string())?;

    let ps_get_next_process = {
        let pattern = ByteSequencePattern::parse(obfstr!("E8 ? ? ? ? 48 8B D8 48 85 C0 74 24 F7"))
            .with_context(|| obfstr!("Failed to compile PsGetNextProcess pattern").to_string())?;

        NtOffsets::locate_function(
            &kernel_base, obfstr!("PsGetNextProcess"), 
            &pattern, 0x01, 0x05
        )?
    };
    
    let mm_verify_callback_function_flags = {
        let pattern = ByteSequencePattern::parse(obfstr!("E8 ?? ?? ?? ?? 85 C0 0F 84 ?? ?? ?? ?? 48 8B 4D 00"))
            .with_context(|| obfstr!("Failed to compile MmVerifyCallbackFunctionFlags pattern").to_string())?;

        NtOffsets::locate_function(
            &kernel_base, obfstr!("MmVerifyCallbackFunctionFlags"), 
            &pattern, 0x01, 0x05
        )?
    };

    let eprocess_thread_list_head = {
        let pattern = ByteSequencePattern::parse(obfstr!("4C 8D A9 ? ? ? ? 33 DB"))
            .with_context(|| obfstr!("Failed to compile _EPROCESS.ThreadListHead pattern").to_string())?;

        NtOffsets::locate_offset(
            &kernel_base, obfstr!("_EPROCESS.ThreadListHead"), 
            &pattern, 0x03,
        )?
    };

    let nt_offsets = unsafe { &mut *NT_OFFSETS.get() };
    *nt_offsets = Some(NtOffsets {
        PsGetNextProcess: ps_get_next_process,
        MmVerifyCallbackFunctionFlags: mm_verify_callback_function_flags,

        EPROCESS_ThreadListHead: eprocess_thread_list_head
    });

    Ok(())
}

impl NtOffsets {
    pub fn locate_function<T>(
        module: &KModule, 
        name: &str,
        pattern: &dyn SearchPattern,
        offset_rel_address: isize,
        instruction_length: usize
    ) -> anyhow::Result<T>
    where
        T: Sized
    {
        let pattern_match = module.find_code_sections()?
            .into_iter()
            .find_map(|section| {
                if let Some(offset) = pattern.find(section.raw_data()) {
                    Some(offset + section.raw_data_address())
                } else {
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

        log::debug!("{}::{} located at {:X}", module.file_name, name, target);
        unsafe { Ok(core::mem::transmute_copy::<_, T>(&target)) }
    }

    pub fn locate_offset(
        module: &KModule, 
        name: &str,
        pattern: &dyn SearchPattern,
        inst_offset: isize,
    ) -> anyhow::Result<usize> {
        let pattern_match = module.find_code_sections()?
            .into_iter()
            .find_map(|section| {
                if let Some(offset) = pattern.find(section.raw_data()) {
                    Some(offset + section.raw_data_address())
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
