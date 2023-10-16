use alloc::{
    string::ToString,
    vec::Vec,
};

use anyhow::{
    anyhow,
    Context,
};
use obfstr::obfstr;
use valthrun_driver_shared::{
    ByteSequencePattern,
    KeyboardState,
};
use winapi::{
    km::wdm::DRIVER_OBJECT,
    shared::ntdef::{
        PVOID,
        UNICODE_STRING,
    },
};

use crate::{
    kapi::{
        KModule,
        Object,
        UnicodeStringEx,
    },
    kdef::{
        IoDriverObjectType,
        KeyboardClassServiceCallbackFn,
        KEYBOARD_FLAG_BREAK,
        KEYBOARD_FLAG_MAKE,
        KEYBOARD_INPUT_DATA,
    },
    offsets::NtOffsets,
    winver::os_info,
};

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
        (self.service_callback)(
            self.kb_device.cast(),
            input_ptr.start,
            input_ptr.end,
            &mut consumed,
        );
    }
}

fn find_keyboard_service_callback() -> anyhow::Result<KeyboardClassServiceCallbackFn> {
    let module_kdbclass = KModule::find_by_name(obfstr!("kbdclass.sys"))?
        .with_context(|| anyhow!("failed to locate {} module", obfstr!("kbdclass.sys")))?;

    let pattern = if os_info().dwBuildNumber >= 22_000 {
        ByteSequencePattern::parse(obfstr!("48 8D 05 ? ? ? ? 48 89 45"))
    } else {
        ByteSequencePattern::parse(obfstr!("48 8D 05 ? ? ? ? 48 89 44 24"))
    }
    .with_context(|| {
        obfstr!("Failed to compile KeyboardClassServiceCallback pattern").to_string()
    })?;

    NtOffsets::locate_function(
        &module_kdbclass,
        obfstr!("KeyboardClassServiceCallback"),
        &pattern,
        0x03,
        0x07,
    )
}

#[allow(unused)]
pub fn create_keyboard_input() -> anyhow::Result<KeyboardInput> {
    let name = UNICODE_STRING::from_bytes(obfstr::wide!("\\Driver\\KbdClass"));
    let kb_driver = Object::reference_by_name(&name, unsafe { *IoDriverObjectType })
        .map_err(|code| anyhow!("{} 0x{:X}", obfstr!("Object::reference_by_name"), code))?;
    let kb_driver = kb_driver.cast::<DRIVER_OBJECT>();

    /* To get all keyboard devices we could use kb_device.NextDevice. Currently we use the first one available. */
    let kb_device = unsafe { kb_driver.DeviceObject.as_mut() };
    let kb_device = match kb_device {
        Some(device) => Object::reference(device as *mut _ as PVOID),
        None => anyhow::bail!("{}", obfstr!("no keyboard device detected")),
    };

    let service_callback = find_keyboard_service_callback()?;

    Ok(KeyboardInput {
        kb_device,
        service_callback,
    })
}
