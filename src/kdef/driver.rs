#[allow(unused)]
use winapi::{shared::ntdef::{UNICODE_STRING, NTSTATUS}, km::wdm::{PDRIVER_OBJECT, PDEVICE_OBJECT}};


#[allow(unused)]
extern "system" {
    pub fn IoCreateDriver(name: *const UNICODE_STRING, entry: *const ()) -> NTSTATUS;
    pub fn IoAttachDeviceToDeviceStack(SourceDevice: PDEVICE_OBJECT, TargetDevice: PDEVICE_OBJECT) -> PDEVICE_OBJECT;
}