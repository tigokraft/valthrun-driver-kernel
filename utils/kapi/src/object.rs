use alloc::{
    string::String,
    vec::Vec,
};

use anyhow::anyhow;
use kdef::{
    OBJECT_NAME_INFORMATION,
    POBJECT_TYPE,
};
use obfstr::obfstr;
use winapi::{
    km::wdm::KPROCESSOR_MODE,
    shared::ntdef::{
        HANDLE,
        NTSTATUS,
        OBJ_CASE_INSENSITIVE,
        PVOID,
        UNICODE_STRING,
    },
    um::winnt::ACCESS_MASK,
};

use crate::{
    imports::{
        ObQueryNameString,
        ObReferenceObjectByHandle,
        ObReferenceObjectByName,
        ObfDereferenceObject,
        ObfReferenceObject,
    },
    NTStatusEx,
    UnicodeStringEx,
};

pub struct Object(PVOID);

unsafe impl Sync for Object {}

impl Object {
    pub fn reference(target: PVOID) -> Self {
        unsafe { ObfReferenceObject(target) };
        Self(target)
    }

    // From a reference instance
    pub fn from_owned(target: PVOID) -> Self {
        Self(target)
    }

    pub fn reference_by_handle(handle: HANDLE, access: ACCESS_MASK) -> Result<Object, NTSTATUS> {
        let mut object: PVOID = core::ptr::null_mut();
        unsafe {
            ObReferenceObjectByHandle(
                handle,
                access,
                core::ptr::null_mut(),
                KPROCESSOR_MODE::KernelMode,
                &mut object as *mut _ as PVOID,
                core::ptr::null_mut(),
            )
            .ok()?
        };

        Ok(Object(object))
    }

    pub fn reference_by_name(
        name: &UNICODE_STRING,
        ob_type: POBJECT_TYPE,
    ) -> Result<Object, NTSTATUS> {
        let mut object: PVOID = core::ptr::null_mut();
        unsafe {
            ObReferenceObjectByName(
                name,
                OBJ_CASE_INSENSITIVE,
                core::ptr::null_mut(),
                0,
                ob_type,
                KPROCESSOR_MODE::KernelMode,
                core::ptr::null_mut(),
                &mut object as *mut _ as PVOID,
            )
            .ok()?
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

pub enum ObjectType {
    IoDriverObjectType,
    PsProcessType,
}

impl ObjectType {
    pub fn resolve_system_type(&self) -> *const POBJECT_TYPE {
        let result = match self {
            Self::IoDriverObjectType => {
                kapi_kmodule::resolve_import(None, obfstr!("IoDriverObjectType"))
            }
            Self::PsProcessType => kapi_kmodule::resolve_import(None, obfstr!("PsProcessType")),
        };

        result.as_ptr() as *const POBJECT_TYPE
    }
}
// dynamic_import_table! {
//     pub imports OBJECT_TYPE_IMPORT {
//         pub CmKeyObjectType: *const POBJECT_TYPE = SystemExport::new(obfstr!("CmKeyObjectType")),
//         pub IoFileObjectType: *const POBJECT_TYPE = SystemExport::new(obfstr!("IoFileObjectType")),
//         pub IoDriverObjectType: *const POBJECT_TYPE = SystemExport::new(obfstr!("IoDriverObjectType")),
//         pub IoDeviceObjectType: *const POBJECT_TYPE = SystemExport::new(obfstr!("IoDeviceObjectType")),
//         pub ExEventObjectType: *const POBJECT_TYPE = SystemExport::new(obfstr!("ExEventObjectType")),
//         pub ExSemaphoreObjectType: *const POBJECT_TYPE = SystemExport::new(obfstr!("ExSemaphoreObjectType")),
//         pub TmTransactionManagerObjectType: *const POBJECT_TYPE = SystemExport::new(obfstr!("TmTransactionManagerObjectType")),
//         pub TmResourceManagerObjectType: *const POBJECT_TYPE = SystemExport::new(obfstr!("TmResourceManagerObjectType")),
//         pub TmEnlistmentObjectType: *const POBJECT_TYPE = SystemExport::new(obfstr!("TmEnlistmentObjectType")),
//         pub TmTransactionObjectType: *const POBJECT_TYPE = SystemExport::new(obfstr!("TmTransactionObjectType")),
//         pub PsProcessType: *const POBJECT_TYPE = SystemExport::new(obfstr!("PsProcessType")),
//         pub PsThreadType: *const POBJECT_TYPE = SystemExport::new(obfstr!("PsThreadType")),
//         pub PsJobType: *const POBJECT_TYPE = SystemExport::new(obfstr!("PsJobType")),
//         pub SeTokenObjectType: *const POBJECT_TYPE = SystemExport::new(obfstr!("SeTokenObjectType")),
//     }
// }
