
use winapi::shared::ntdef::{UNICODE_STRING, NTSTATUS};


#[allow(unused)]
extern "system" {
    pub fn IoCreateDriver(name: *const UNICODE_STRING, entry: *const ()) -> NTSTATUS;
}