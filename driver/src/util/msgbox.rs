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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[repr(C)]
pub enum BoxButtons {
    Ok = 0,
    OkCancel = 1,
    AbortRetryIgnore = 2,
    YesNoCancel = 3,
    YesNo = 4,
    RetryCancel = 5,
    CancelTryContinue = 6,
}

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

pub fn show_msgbox(title: &str, message: &str, buttons: BoxButtons) -> ErrorResponse {
    let title = title.encode_utf16().collect::<Vec<_>>();
    let title = UNICODE_STRING::from_bytes_unchecked(&title);

    let message = message.encode_utf16().collect::<Vec<_>>();
    let message = UNICODE_STRING::from_bytes_unchecked(&message);

    let parameters = [
        &message as *const _ as *const c_void,
        &title as *const _ as *const c_void,
        buttons as u32 as *const c_void,
    ];

    let mut response = ErrorResponse::Invalid;
    let status =
        unsafe { ExRaiseHardError(0x50000018, 3, 0x03, parameters.as_ptr(), 1, &mut response) };
    log::trace!("Status = {status:X}");

    response
}
