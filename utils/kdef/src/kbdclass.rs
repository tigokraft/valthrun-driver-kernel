#![allow(unused)]
use winapi::km::wdm::PDEVICE_OBJECT;

pub const KEYBOARD_FLAG_MAKE: u16 = 0x00;
pub const KEYBOARD_FLAG_BREAK: u16 = 0x01;
pub const KEYBOARD_FLAG_E0: u16 = 0x02;
pub const KEYBOARD_FLAG_E1: u16 = 0x04;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
#[allow(non_camel_case_types, non_snake_case)]
pub struct KEYBOARD_INPUT_DATA {
    pub UnitId: u16,
    pub MakeCode: u16,
    pub Flags: u16,
    pub Reserved: u16,
    pub ExtraInformation: u16,
}

pub type KeyboardClassServiceCallbackFn = extern "system" fn(
    DeviceObject: PDEVICE_OBJECT,
    InputDataStart: *const KEYBOARD_INPUT_DATA,
    InputDataEnd: *const KEYBOARD_INPUT_DATA,
    InputDataConsumed: *mut u32,
) -> ();
