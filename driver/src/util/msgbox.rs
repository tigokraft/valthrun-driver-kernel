use alloc::vec::Vec;

use kapi::{
    Process,
    UnicodeStringEx,
};
use lazy_link::lazy_link;
use winapi::{
    ctypes::c_void,
    shared::ntdef::{
        NTSTATUS,
        UNICODE_STRING,
    },
};

#[lazy_link(resolver = "kapi_kmodule::resolve_import")]
extern "C" {
    #[allow(non_snake_case)]
    fn ExRaiseHardError(
        error_status: u64,
        number_of_parameters: u64,
        parameter_unicode_mask: u64,
        parameters: *const *const c_void,
        response_option: u32,
        response: *mut ErrorResponse,
    ) -> NTSTATUS;
}

pub const MB_OK: u32 = 0x00000000;
pub const MB_OKCANCEL: u32 = 0x00000001;
pub const MB_ABORTRETRYIGNORE: u32 = 0x00000002;
pub const MB_YESNOCANCEL: u32 = 0x00000003;
pub const MB_YESNO: u32 = 0x00000004;
pub const MB_RETRYCANCEL: u32 = 0x00000005;
pub const MB_CANCELTRYCONTINUE: u32 = 0x00000006;

pub const MB_ICONERROR: u32 = 0x00000010;
pub const MB_ICONEXCLAMATION: u32 = 0x00000030;

pub const MB_DEFBUTTON3: u32 = 0x00000200;

pub const MB_SYSTEMMODAL: u32 = 0x00001000;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[repr(C)]
pub enum ErrorResponse {
    ReturnToCaller = 0,
    NotHandled = 1,
    Abort = 2,
    Cancel = 3,
    Ignore = 4,
    No = 5,
    Ok = 6,
    Retry = 7,
    Yes = 8,

    Invalid = 0xFF,
}

fn is_msgbox_supported() -> bool {
    Process::current().get_id() >= 0x04
}

pub fn show_msgbox(title: &str, message: &str, buttons: u32) -> ErrorResponse {
    let title = title.encode_utf16().collect::<Vec<_>>();
    let title = UNICODE_STRING::from_bytes_unchecked(&title);

    let message = message.encode_utf16().collect::<Vec<_>>();
    let message = UNICODE_STRING::from_bytes_unchecked(&message);

    let parameters = [
        &message as *const _ as *const c_void,
        &title as *const _ as *const c_void,
        buttons as *const c_void,
    ];

    let mut response = ErrorResponse::Invalid;
    let status =
        unsafe { ExRaiseHardError(0x50000018, 3, 0x03, parameters.as_ptr(), 1, &mut response) };
    log::trace!("Status = {status:X}");

    response
}
