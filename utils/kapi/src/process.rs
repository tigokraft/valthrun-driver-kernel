use alloc::vec::Vec;
use core::ffi::CStr;

use kdef::{
    _KAPC_STATE,
    _LDR_DATA_TABLE_ENTRY,
};
use valthrun_driver_shared::ModuleInfo;
use winapi::{
    km::wdm::PEPROCESS,
    shared::ntdef::{
        NT_SUCCESS,
        PVOID,
    },
};

use super::Object;
use crate::{
    imports::{
        KeStackAttachProcess,
        KeUnstackDetachProcess,
        PsGetCurrentProcess,
        PsGetProcessId,
        PsGetProcessImageFileName,
        PsGetProcessPeb,
        PsLookupProcessByProcessId,
    },
    UnicodeStringEx,
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
        Self::from_raw(unsafe { PsGetCurrentProcess() }, false)
    }

    pub fn by_id(process_id: i32) -> Option<Self> {
        let mut process = core::ptr::null_mut();

        let status = unsafe { PsLookupProcessByProcessId(process_id as _, &mut process) };
        if NT_SUCCESS(status) {
            Some(Self::from_raw(process, true))
        } else {
            None
        }
    }

    pub fn get_id(&self) -> i32 {
        unsafe { PsGetProcessId(self.eprocess()) }
    }

    /// Process image file name (max 14 characters log)!
    pub fn get_image_file_name(&self) -> Option<&str> {
        unsafe {
            CStr::from_ptr(PsGetProcessImageFileName(self.eprocess()))
                .to_str()
                .ok()
        }
    }

    pub fn attach(&self) -> AttachedProcess {
        let mut apc_state: _KAPC_STATE = unsafe { core::mem::zeroed() };
        unsafe { KeStackAttachProcess(self.eprocess(), &mut apc_state) };
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

    pub fn get_modules(&self) -> Vec<ModuleInfo> {
        let mut result = Vec::with_capacity(64);

        let peb = match unsafe { PsGetProcessPeb(self.process.eprocess()).as_ref() } {
            Some(peb) => peb,
            None => {
                log::warn!("Failed to get PEB for {:X}", self.process.eprocess() as u64);
                return result;
            }
        };

        let ldr = match unsafe { peb.Ldr.as_ref() } {
            Some(ldr) => ldr,
            None => {
                log::warn!(
                    "Missing process module list for {:X}",
                    self.process.eprocess() as u64
                );
                return result;
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
            let mut module_info = ModuleInfo {
                base_dll_name: [0u8; 0xFF],
                base_address: entry.DllBase as usize,
                module_size: entry.SizeOfImage as usize,
            };

            let name_bytes = base_name.as_bytes();
            let name_bytes_length = module_info.base_dll_name.len().min(name_bytes.len());
            module_info.base_dll_name[0..name_bytes_length]
                .copy_from_slice(&name_bytes[0..name_bytes_length]);

            result.push(module_info);
            current_entry = unsafe { (*current_entry).Flink };
        }

        result
    }
}

impl Drop for AttachedProcess<'_> {
    fn drop(&mut self) {
        unsafe { KeUnstackDetachProcess(&mut self.apc_state) };
    }
}
