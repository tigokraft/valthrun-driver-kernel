#![allow(non_snake_case)]

use lazy_link::lazy_link;
use winapi::shared::ntdef::{
    NTSTATUS,
    UNICODE_STRING,
};

#[lazy_link(resolver = "kapi_kmodule::resolve_import")]
extern "C" {
    pub fn IoCreateDriver(name: *const UNICODE_STRING, entry: *const ()) -> NTSTATUS;
    //pub fn DbgPrintEx(ComponentId: u32, Level: u32, Format: *const u8, ...) -> NTSTATUS;
    pub fn DbgBreakPoint();
    pub fn KeBugCheck(code: u32) -> !;
}
