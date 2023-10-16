use winapi::shared::ntdef::{
    NTSTATUS,
    PVOID,
    UNICODE_STRING,
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
    pub fn ObRegisterCallbacks(
        CallbackRegistration: *const _OB_CALLBACK_REGISTRATION,
        RegistrationHandle: *mut PVOID,
    ) -> NTSTATUS;
    pub fn ObUnRegisterCallbacks(RegistrationHandle: PVOID);
    pub fn ObGetFilterVersion() -> u16;

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
