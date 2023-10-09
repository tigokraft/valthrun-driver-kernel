#[allow(unused)]
use winapi::{
    km::wdm::{
        PDEVICE_OBJECT,
        PDRIVER_OBJECT,
    },
    shared::ntdef::{
        NTSTATUS,
        UNICODE_STRING,
    },
};

#[allow(unused)]
extern "system" {
    pub fn IoCreateDriver(name: *const UNICODE_STRING, entry: *const ()) -> NTSTATUS;
    pub fn IoAttachDeviceToDeviceStack(
        SourceDevice: PDEVICE_OBJECT,
        TargetDevice: PDEVICE_OBJECT,
    ) -> PDEVICE_OBJECT;
}
