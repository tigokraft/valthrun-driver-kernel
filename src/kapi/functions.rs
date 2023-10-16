use winapi::shared::ntdef::UNICODE_STRING;

use super::UnicodeStringEx;
use crate::kdef::MmGetSystemRoutineAddress;

pub fn get_system_routine<T: Sized>(name: &'static [u16]) -> Option<T> {
    let uname = UNICODE_STRING::from_bytes(name);
    unsafe {
        let address = MmGetSystemRoutineAddress(&uname);
        if address.is_null() {
            None
        } else {
            Some(core::mem::transmute_copy(&address))
        }
    }
}
