use alloc::{
    string::{
        String,
        ToString,
    },
    vec::Vec,
};
use core::{
    ffi::CStr,
    mem::size_of,
};

use anyhow::Context;
use obfstr::obfstr;
use utils_pattern::SearchPattern;
use winapi::{
    shared::{
        ntdef::PVOID,
        ntstatus::STATUS_SUCCESS,
    },
    um::winnt::{
        IMAGE_FILE_HEADER,
        IMAGE_SCN_CNT_CODE,
        IMAGE_SECTION_HEADER,
    },
};

use crate::{
    def::{
        SystemModuleInformation,
        _SYSTEM_MODULE_ENTRY,
        _SYSTEM_MODULE_INFORMATION,
    },
    imports::{
        MmIsAddressValid,
        RtlImageNtHeader,
        ZwQuerySystemInformation,
    },
};

pub struct KModuleSection {
    pub name: String,
    pub module_base: usize,
    pub virtual_address: usize,
    pub size_of_raw_data: usize,
    pub characteristics: u32,
}

impl KModuleSection {
    fn from_header(header: &IMAGE_SECTION_HEADER, module: &KModule) -> Self {
        let section_name = CStr::from_bytes_until_nul(&header.Name)
            .unwrap_or_default()
            .to_string_lossy();

        Self {
            name: section_name.to_string(),
            module_base: module.base_address,
            virtual_address: header.VirtualAddress as usize,
            size_of_raw_data: header.SizeOfRawData as usize,
            characteristics: header.Characteristics,
        }
    }

    pub fn is_data_valid(&self) -> bool {
        unsafe { MmIsAddressValid(self.raw_data_address() as *const () as PVOID) }
    }

    pub fn raw_data_unchecked(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                self.raw_data_address() as *const u8,
                self.size_of_raw_data as usize,
            )
        }
    }

    pub fn raw_data(&self) -> Option<&[u8]> {
        if !self.is_data_valid() {
            return None;
        }

        Some(self.raw_data_unchecked())
    }

    pub fn raw_data_address(&self) -> usize {
        self.module_base + self.virtual_address
    }

    /// Search for a pattern in the current section.
    /// ATTENTION: The result is the **absolute** address.
    pub fn find_pattern(&self, pattern: &dyn SearchPattern) -> Option<usize> {
        let offset = pattern.find(self.raw_data()?)?;
        Some(self.raw_data_address() + offset)
    }
}

pub struct KModule {
    pub file_path: String,
    pub file_name: String,
    pub base_address: usize,
    pub module_size: usize,
}

impl KModule {
    fn from_module_entry(entry: &_SYSTEM_MODULE_ENTRY) -> Self {
        let file_path = CStr::from_bytes_until_nul(&entry.FullPathName)
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let file_name =
            CStr::from_bytes_until_nul(&entry.FullPathName[entry.OffsetToFileName as usize..])
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

        Self {
            file_path,
            file_name,
            base_address: entry.ImageBase as usize,
            module_size: entry.ImageSize as usize,
        }
    }
}

impl KModule {
    pub fn is_base_data_valid(&self) -> bool {
        unsafe { MmIsAddressValid(self.base_address as *const () as PVOID) }
    }

    fn section_headers(&self) -> anyhow::Result<&'static [IMAGE_SECTION_HEADER]> {
        let header = unsafe { RtlImageNtHeader(self.base_address as PVOID).as_mut() }
            .with_context(|| obfstr!("RtlImageNtHeader failed").to_string())?;

        let section_headers = (&header.FileHeader as *const _ as *const ())
            .wrapping_byte_add(size_of::<IMAGE_FILE_HEADER>())
            .wrapping_byte_add(header.FileHeader.SizeOfOptionalHeader as usize)
            .cast::<IMAGE_SECTION_HEADER>();

        Ok(unsafe {
            core::slice::from_raw_parts(
                section_headers,
                header.FileHeader.NumberOfSections as usize,
            )
        })
    }

    pub fn find_sections_by_name(&self, name: &str) -> anyhow::Result<Vec<KModuleSection>> {
        let result = self
            .section_headers()?
            .iter()
            .map(|section| KModuleSection::from_header(section, self))
            .filter(|section| section.name == name)
            .collect::<Vec<_>>();

        Ok(result)
    }

    pub fn find_code_sections(&self) -> anyhow::Result<Vec<KModuleSection>> {
        Ok(self
            .section_headers()?
            .iter()
            .filter(|section| (section.Characteristics & IMAGE_SCN_CNT_CODE) > 0)
            .map(|section| KModuleSection::from_header(section, self))
            .collect::<Vec<_>>())
    }

    pub fn query_modules() -> anyhow::Result<Vec<KModule>> {
        unsafe {
            let mut bytes = 0;
            ZwQuerySystemInformation(
                SystemModuleInformation,
                core::ptr::null_mut(),
                0,
                &mut bytes,
            );

            let mut buffer = Vec::<u8>::with_capacity(bytes as usize);
            buffer.set_len(bytes as usize);

            let status = ZwQuerySystemInformation(
                SystemModuleInformation,
                buffer.as_mut_ptr() as *mut (),
                bytes,
                &mut bytes,
            );
            if status != STATUS_SUCCESS {
                anyhow::bail!(
                    "{} -> {:X}",
                    obfstr!("ZwQuerySystemInformation query"),
                    status
                )
            }

            let info =
                &*core::mem::transmute::<_, *const _SYSTEM_MODULE_INFORMATION>(buffer.as_ptr());
            Ok(info
                .modules()
                .iter()
                .map(KModule::from_module_entry)
                .collect())
        }
    }

    pub fn find_by_name(target: &str) -> anyhow::Result<Option<KModule>> {
        for module in Self::query_modules()? {
            if module.file_name == target {
                return Ok(Some(module));
            }
        }

        Ok(None)
    }
}
