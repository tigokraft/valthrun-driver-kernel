use alloc::{string::String, vec::Vec};
use anyhow::anyhow;
use winapi::{shared::ntdef::{PVOID, UNICODE_STRING, OBJ_CASE_INSENSITIVE, NTSTATUS}, km::wdm::KPROCESSOR_MODE};

use crate::{kdef::{ObfDereferenceObject, ObReferenceObjectByName, POBJECT_TYPE, OBJECT_NAME_INFORMATION, ObQueryNameString, ObfReferenceObject}, kapi::UnicodeStringEx};

use super::NTStatusEx;

pub struct Object(PVOID);

unsafe impl Sync for Object {}

impl Object {
    pub fn reference(target: PVOID) -> Self {
        unsafe { ObfReferenceObject(target) };
        Self(target)
    }

    pub fn reference_by_name(name: &UNICODE_STRING, ob_type: POBJECT_TYPE) -> Result<Object, NTSTATUS> {
        let mut object: PVOID = core::ptr::null_mut();
        unsafe {
            ObReferenceObjectByName(
                name, OBJ_CASE_INSENSITIVE, 
                core::ptr::null_mut(), 0, 
                ob_type, 
                KPROCESSOR_MODE::KernelMode, core::ptr::null_mut(), 
                &mut object as *mut _ as PVOID,
            ).ok()?
        };

        Ok(Object(object))
    }

    pub fn cast<T>(&self) -> &mut T {
        unsafe { &mut *(self.0 as *mut T) }
    }

    pub fn name(&self) -> anyhow::Result<String> {
        let mut buffer = Vec::<u8>::with_capacity(1024);
        buffer.resize(1024, 0);

        let name_info = buffer.as_mut_ptr() as *mut OBJECT_NAME_INFORMATION;

        let mut name_length = 0;
        unsafe {
            ObQueryNameString(self.0, name_info, buffer.len() as u32, &mut name_length)
                .ok()
                .map_err(|err| anyhow!("ObQueryNameString {:X}", err))?;
        }

        log::debug!("Name length: {}", name_length);
        Ok(unsafe { &*(name_info) }.Name.as_string_lossy())
    }

    pub fn drop_defer_delete(self) {
        todo!();
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                ObfDereferenceObject(self.0);
            }
        }
    }
}