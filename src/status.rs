//! Custom defined status codes by the driver

use winapi::shared::ntdef::NTSTATUS;

const fn custom_status(code: u32) -> NTSTATUS {
    (code + 0xCF000000) as i32
}

pub const CSTATUS_LOG_INIT_FAILED: NTSTATUS = custom_status(0x01);
pub const CSTATUS_DRIVER_PREINIT_FAILED: NTSTATUS = custom_status(0x02);
pub const CSTATUS_DRIVER_INIT_FAILED: NTSTATUS = custom_status(0x03);
pub const CSTATUS_DRIVER_ALREADY_LOADED: NTSTATUS = custom_status(0x04);
