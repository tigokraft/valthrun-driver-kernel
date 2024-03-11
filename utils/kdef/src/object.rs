use winapi::shared::ntdef::UNICODE_STRING;

pub type _OBJECT_TYPE = ();
pub type POBJECT_TYPE = *const _OBJECT_TYPE;

#[repr(C)]
#[derive(Default)]
pub struct OBJECT_NAME_INFORMATION {
    pub Name: UNICODE_STRING,
}
