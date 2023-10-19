use core::ffi::CStr;

use kdef::{
    _KAPC_STATE,
    _LDR_DATA_TABLE_ENTRY,
};
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
use crate::GLOBAL_IMPORTS;

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

pub struct ModuleInfo {
    pub base_address: usize,
    pub module_size: usize,
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
