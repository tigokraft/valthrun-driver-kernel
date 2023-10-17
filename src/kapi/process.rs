use alloc::vec::Vec;
use core::ffi::CStr;

use valthrun_driver_shared::ModuleInfo;
use winapi::{
    km::wdm::PEPROCESS,
    shared::ntdef::{
        NT_SUCCESS,
        PVOID,
    },
};

use super::{
    Object,
    UnicodeStringEx,
};
use crate::{
    imports::GLOBAL_IMPORTS,
    kdef::{
        _KAPC_STATE,
        _LDR_DATA_TABLE_ENTRY,
    },
    offsets::get_nt_offsets,
};

pub struct Process {
    inner: Object,
}

impl Process {
    pub fn eprocess(&self) -> PEPROCESS {
        self.inner.cast()
    }

    pub fn from_raw(eprocess: PEPROCESS, owns_reference: bool) -> Self {
        Self {
            inner: if owns_reference {
                Object::from_owned(eprocess as PVOID)
            } else {
                Object::reference(eprocess as PVOID)
            },
        }
    }

    pub fn current() -> Process {
        let imports = GLOBAL_IMPORTS.unwrap();
        Self::from_raw(unsafe { (imports.PsGetCurrentProcess)() }, false)
    }

    pub fn by_id(process_id: i32) -> Option<Self> {
        let mut process = core::ptr::null_mut();

        let imports = GLOBAL_IMPORTS.unwrap();
        let status = unsafe { (imports.PsLookupProcessByProcessId)(process_id as _, &mut process) };
        if NT_SUCCESS(status) {
            Some(Self::from_raw(process, true))
        } else {
            None
        }
    }

    pub fn get_id(&self) -> i32 {
        let imports = GLOBAL_IMPORTS.unwrap();
        unsafe { (imports.PsGetProcessId)(self.eprocess()) }
    }

    /// Process image file name (max 14 characters log)!
    pub fn get_image_file_name(&self) -> Option<&str> {
        let imports = GLOBAL_IMPORTS.resolve().ok()?;

        unsafe {
            CStr::from_ptr((imports.PsGetProcessImageFileName)(self.eprocess()))
                .to_str()
                .ok()
        }
    }

    pub fn attach(&self) -> AttachedProcess {
        let imports = GLOBAL_IMPORTS.unwrap();

        let mut apc_state: _KAPC_STATE = unsafe { core::mem::zeroed() };
        unsafe { (imports.KeStackAttachProcess)(self.eprocess(), &mut apc_state) };
        AttachedProcess {
            process: self,
            apc_state,
        }
    }

    pub fn get_directory_table_base(&self) -> u64 {
        unsafe { *self.eprocess().byte_offset(0x28).cast::<u64>() }
    }
}

pub struct AttachedProcess<'a> {
    process: &'a Process,
    apc_state: _KAPC_STATE,
}

impl AttachedProcess<'_> {
    pub fn get_id(&self) -> i32 {
        self.process.get_id()
    }

    pub fn get_module(&self, name: &str) -> Option<ModuleInfo> {
        let imports = GLOBAL_IMPORTS.unwrap();
        let peb = match unsafe { (imports.PsGetProcessPeb)(self.process.eprocess()).as_ref() } {
            Some(peb) => peb,
            None => {
                log::warn!("Failed to get PEB for {:X}", self.process.eprocess() as u64);
                return None;
            }
        };

        let ldr = match unsafe { peb.Ldr.as_ref() } {
            Some(ldr) => ldr,
            None => {
                log::warn!(
                    "Missing process module list for {:X}",
                    self.process.eprocess() as u64
                );
                return None;
            }
        };

        let mut current_entry = ldr.InLoadOrderModuleList.Flink as *const _;
        while current_entry != &ldr.InLoadOrderModuleList {
            let entry = unsafe {
                current_entry
                    .byte_offset(0) /* InLoadOrderLinks is the first entry */
                    .cast::<_LDR_DATA_TABLE_ENTRY>()
                    .read()
            };
            let base_name = entry.BaseDllName.as_string_lossy();
            if base_name == name {
                return Some(ModuleInfo {
                    base_address: entry.DllBase as usize,
                    module_size: entry.SizeOfImage as usize,
                });
            }

            current_entry = unsafe { (*current_entry).Flink };
        }

        None
    }
}

impl Drop for AttachedProcess<'_> {
    fn drop(&mut self) {
        let imports = GLOBAL_IMPORTS.unwrap();
        unsafe { (imports.KeUnstackDetachProcess)(&mut self.apc_state) };
    }
}

pub fn find_processes_by_name(target_name: &str) -> anyhow::Result<Vec<Process>> {
    let imports = GLOBAL_IMPORTS.resolve()?;

    #[allow(non_snake_case)]
    let PsGetNextProcess = get_nt_offsets().PsGetNextProcess;

    #[allow(non_snake_case)]
    let EPROCESS_ThreadListHead = get_nt_offsets().EPROCESS_ThreadListHead;

    let mut cs2_candidates = Vec::with_capacity(8);

    let mut current_peprocess = core::ptr::null_mut();
    loop {
        current_peprocess = unsafe { PsGetNextProcess(current_peprocess) };
        if current_peprocess.is_null() {
            break;
        }

        let image_file_name = unsafe {
            CStr::from_ptr((imports.PsGetProcessImageFileName)(current_peprocess))
                .to_str()
                .ok()
        };

        if image_file_name != Some(target_name) {
            continue;
        }

        let active_threads = unsafe {
            current_peprocess
                /* The ActiveThreads comes after the thread list head. Thread list head has a size of 0x10. */
                .byte_offset(EPROCESS_ThreadListHead as isize + 0x10)
                .cast::<u32>()
                .read_volatile()
        };

        log::trace!(
            "{} matched {:X}: {:?} ({})",
            target_name,
            current_peprocess as u64,
            image_file_name,
            active_threads
        );
        if active_threads == 0 {
            /* Process terminated / not running */
            continue;
        }

        cs2_candidates.push(Process::from_raw(current_peprocess, false));
    }

    Ok(cs2_candidates)
}
