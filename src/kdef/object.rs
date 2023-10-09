use winapi::{
    km::wdm::KPROCESSOR_MODE,
    shared::ntdef::{
        NTSTATUS,
        PCUNICODE_STRING,
        PVOID,
        UNICODE_STRING,
    },
    um::winnt::ACCESS_MASK,
};

use super::_OB_CALLBACK_REGISTRATION;

pub type _OBJECT_TYPE = ();
pub type POBJECT_TYPE = *const _OBJECT_TYPE;

#[repr(C)]
#[derive(Default)]
pub struct OBJECT_NAME_INFORMATION {
    pub Name: UNICODE_STRING,
}

#[allow(unused)]
extern "system" {
    pub fn ObQueryNameString(
        Object: PVOID,
        ObjectNameInfo: *mut OBJECT_NAME_INFORMATION,
        Length: u32,
        ReturnLength: &mut u32,
    ) -> NTSTATUS;

    pub fn ObfDereferenceObject(object: PVOID);
    pub fn ObfReferenceObject(object: PVOID);

    pub fn ObRegisterCallbacks(
        CallbackRegistration: *const _OB_CALLBACK_REGISTRATION,
        RegistrationHandle: *mut PVOID,
    ) -> NTSTATUS;
    pub fn ObUnRegisterCallbacks(RegistrationHandle: PVOID);
    pub fn ObGetFilterVersion() -> u16;

    pub fn ObReferenceObjectByName(
        ObjectName: PCUNICODE_STRING,
        Attributes: u32,
        AccessState: *mut (),
        DesiredAccess: ACCESS_MASK,
        ObjectType: POBJECT_TYPE,
        AccessMode: KPROCESSOR_MODE,
        ParseContext: PVOID,
        Object: PVOID,
    ) -> NTSTATUS;

    pub static CmKeyObjectType: *const POBJECT_TYPE;
    pub static IoFileObjectType: *const POBJECT_TYPE;
    pub static IoDriverObjectType: *const POBJECT_TYPE;
    pub static IoDeviceObjectType: *const POBJECT_TYPE;
    pub static ExEventObjectType: *const POBJECT_TYPE;
    pub static ExSemaphoreObjectType: *const POBJECT_TYPE;
    pub static TmTransactionManagerObjectType: *const POBJECT_TYPE;
    pub static TmResourceManagerObjectType: *const POBJECT_TYPE;
    pub static TmEnlistmentObjectType: *const POBJECT_TYPE;
    pub static TmTransactionObjectType: *const POBJECT_TYPE;
    pub static PsProcessType: *const POBJECT_TYPE;
    pub static PsThreadType: *const POBJECT_TYPE;
    pub static PsJobType: *const POBJECT_TYPE;
    pub static SeTokenObjectType: *const POBJECT_TYPE;
}
