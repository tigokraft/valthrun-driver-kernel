#![allow(unused)]
use winapi::km::wdm::PDEVICE_OBJECT;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
#[allow(non_camel_case_types, non_snake_case)]
pub struct MOUSE_INPUT_DATA {
    pub UnitId: u16,
    pub Flags: u16,
    pub ButtonFlags: u16,
    pub ButtonData: u16,
    pub RawButtons: u32,
    pub LastX: i32,
    pub LastY: i32,
    pub ExtraInformation: u32
}

pub type MouseClassServiceCallbackFn = extern "system" fn(
    DeviceObject: PDEVICE_OBJECT, 
    InputDataStart: *const MOUSE_INPUT_DATA, 
    InputDataEnd: *const MOUSE_INPUT_DATA,
    InputDataConsumed: *mut u32
) -> ();

pub const MOUSE_FLAG_MOVE_RELATIVE: u16 = 0x00;
pub const MOUSE_FLAG_MOVE_ABSOLUTE: u16 = 0x01;
pub const MOUSE_FLAG_VIRTUAL_DESKTOP: u16 = 0x02;
pub const MOUSE_ATTRIBUTES_CHANGED: u16 = 0x04;

pub const MOUSE_BUTTON_LEFT_DOWN: u16 = 0x0001;
pub const MOUSE_BUTTON_LEFT_UP: u16 = 0x0002;
pub const MOUSE_BUTTON_RIGHT_DOWN: u16 = 0x0004;
pub const MOUSE_BUTTON_RIGHT_UP: u16 = 0x0008;
pub const MOUSE_BUTTON_MIDDLE_DOWN: u16 = 0x0010;
pub const MOUSE_BUTTON_MIDDLE_UP: u16 = 0x0020;
pub const MOUSE_BUTTON_4_DOWN: u16 = 0x0040;
pub const MOUSE_BUTTON_4_UP: u16 = 0x0080;
pub const MOUSE_BUTTON_5_DOWN: u16 = 0x0100;
pub const MOUSE_BUTTON_5_UP: u16 = 0x0200;
pub const MOUSE_BUTTON_WHEEL: u16 = 0x0400;
pub const MOUSE_BUTTON_HWHEEL: u16 = 0x0800;