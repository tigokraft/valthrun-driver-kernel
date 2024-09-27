use alloc::{
    string::ToString,
    vec::Vec,
};

use anyhow::{
    anyhow,
    Context,
};
use kapi::{
    KeLowerIrql,
    KeRaiseIrql,
    Object,
    ObjectType,
    UnicodeStringEx,
    DISPATCH_LEVEL,
};
use kapi_kmodule::KModule;
use kdef::{
    KeyboardClassServiceCallbackFn,
    KEYBOARD_FLAG_BREAK,
    KEYBOARD_FLAG_MAKE,
    KEYBOARD_INPUT_DATA,
};
use obfstr::obfstr;
use valthrun_driver_shared::{
    KeyboardState,
    Signature,
};
use winapi::{
    km::wdm::DRIVER_OBJECT,
    shared::ntdef::{
        PVOID,
        UNICODE_STRING,
    },
};

use crate::offsets::NtOffsets;

pub struct KeyboardInput {
    kb_device: Object,
    service_callback: KeyboardClassServiceCallbackFn,
}

fn keyboard_state_to_input(state: &KeyboardState) -> KEYBOARD_INPUT_DATA {
    let mut input_data: KEYBOARD_INPUT_DATA = Default::default();
    input_data.MakeCode = state.scane_code;
    input_data.Flags = if state.down {
        KEYBOARD_FLAG_MAKE
    } else {
        KEYBOARD_FLAG_BREAK
    };
    input_data
}

impl KeyboardInput {
    pub fn send_input(&self, state: &[KeyboardState]) {
        let input_data = state
            .iter()
            .map(keyboard_state_to_input)
            .collect::<Vec<_>>();

        let mut consumed = 0;
        let input_ptr = input_data.as_ptr_range();

        let irql = KeRaiseIrql(DISPATCH_LEVEL);
        (self.service_callback)(
            self.kb_device.cast(),
            input_ptr.start,
            input_ptr.end,
            &mut consumed,
        );
        KeLowerIrql(irql);
        if consumed > 0 {
            log::debug!("Consumed: {}/{}", consumed, state.len());
        }
    }
}

fn find_keyboard_service_callback() -> anyhow::Result<KeyboardClassServiceCallbackFn> {
    let module_kdbclass = KModule::find_by_name(obfstr!("kbdclass.sys"))?
        .with_context(|| anyhow!("failed to locate {} module", obfstr!("kbdclass.sys")))?;

    [
        /* Windows 11 */
        Signature::relative_address(
            obfstr!("KeyboardClassServiceCallback (>= 22000)"),
            obfstr!("48 8D 05 ? ? ? ? 48 89 45"),
            0x03,
            0x07,
        ),
        /* Windows 11 */
        Signature::relative_address(
            obfstr!("KeyboardClassServiceCallback (< 22000)"),
            obfstr!("48 8D 05 ? ? ? ? 48 89 44 24"),
            0x03,
            0x07,
        ),
    ]
    .iter()
    .find_map(|sig| NtOffsets::locate_signature(&module_kdbclass, sig).ok())
    .map(|v| unsafe { core::mem::transmute_copy(&v) })
    .with_context(|| obfstr!("Failed to find KeyboardClassServiceCallback").to_string())
}

#[allow(unused)]
pub fn create_keyboard_input() -> anyhow::Result<KeyboardInput> {
    let service_callback = find_keyboard_service_callback()?;

    let name = UNICODE_STRING::from_bytes(obfstr::wide!("\\Driver\\KbdClass"));
    let kb_driver = Object::reference_by_name(&name, unsafe {
        *ObjectType::IoDriverObjectType.resolve_system_type()
    })
    .map_err(|code| anyhow!("{} 0x{:X}", obfstr!("Object::reference_by_name"), code))?;
    let kb_driver = kb_driver.cast::<DRIVER_OBJECT>();

    /* To get all keyboard devices we could use kb_device.NextDevice. Currently we use the first one available. */
    let mut kb_device = unsafe { kb_driver.DeviceObject.as_mut() }
        .with_context(|| obfstr!("no keyboard device detected").to_string())?;

    log::debug!("Initial KB device {:X}", kb_device as *mut _ as u64);
    // while let Some(device) = unsafe { kb_device.NextDevice.as_mut() } {
    //     log::debug!(" {:X} -> {:X}", kb_device as *mut _ as u64, device as *mut _ as u64);
    //     kb_device = device;
    // }

    Ok(KeyboardInput {
        kb_device: Object::reference(kb_device as *mut _ as PVOID),
        service_callback,
    })
}
