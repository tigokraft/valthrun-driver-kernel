use alloc::{string::ToString, vec::Vec};
use anyhow::{anyhow, Context};
use obfstr::obfstr;
use valthrun_driver_shared::{ByteSequencePattern, MouseState};
use winapi::{
    km::wdm::DRIVER_OBJECT,
    shared::ntdef::{PVOID, UNICODE_STRING},
};

use crate::{
    kapi::{KModule, Object, UnicodeStringEx},
    kdef::{
        IoDriverObjectType, MouseClassServiceCallbackFn, MOUSE_BUTTON_4_DOWN, MOUSE_BUTTON_4_UP,
        MOUSE_BUTTON_5_DOWN, MOUSE_BUTTON_5_UP, MOUSE_BUTTON_HWHEEL, MOUSE_BUTTON_LEFT_DOWN,
        MOUSE_BUTTON_LEFT_UP, MOUSE_BUTTON_MIDDLE_DOWN, MOUSE_BUTTON_MIDDLE_UP,
        MOUSE_BUTTON_RIGHT_DOWN, MOUSE_BUTTON_RIGHT_UP, MOUSE_BUTTON_WHEEL, MOUSE_INPUT_DATA,
    },
    offsets::NtOffsets,
};

pub struct MouseInput {
    mouse_device: Object,
    service_callback: MouseClassServiceCallbackFn,
}

fn mouse_state_to_input(state: &MouseState) -> MOUSE_INPUT_DATA {
    let mut input_data: MOUSE_INPUT_DATA = Default::default();
    input_data.UnitId = 0;

    if let Some(state) = &state.buttons[0] {
        input_data.ButtonFlags |= if *state {
            MOUSE_BUTTON_LEFT_DOWN
        } else {
            MOUSE_BUTTON_LEFT_UP
        };
    }
    if let Some(state) = &state.buttons[1] {
        input_data.ButtonFlags |= if *state {
            MOUSE_BUTTON_RIGHT_DOWN
        } else {
            MOUSE_BUTTON_RIGHT_UP
        };
    }
    if let Some(state) = &state.buttons[2] {
        input_data.ButtonFlags |= if *state {
            MOUSE_BUTTON_MIDDLE_DOWN
        } else {
            MOUSE_BUTTON_MIDDLE_UP
        };
    }
    if let Some(state) = &state.buttons[3] {
        input_data.ButtonFlags |= if *state {
            MOUSE_BUTTON_4_DOWN
        } else {
            MOUSE_BUTTON_4_UP
        };
    }
    if let Some(state) = &state.buttons[4] {
        input_data.ButtonFlags |= if *state {
            MOUSE_BUTTON_5_DOWN
        } else {
            MOUSE_BUTTON_5_UP
        };
    }
    if state.wheel {
        input_data.ButtonFlags |= MOUSE_BUTTON_WHEEL;
    }
    if state.hwheel {
        input_data.ButtonFlags |= MOUSE_BUTTON_HWHEEL;
    }
    input_data.LastX = state.last_x;
    input_data.LastY = state.last_y;
    input_data
}

impl MouseInput {
    pub fn send_state(&self, state: &[MouseState]) {
        let input_data = state.iter().map(mouse_state_to_input).collect::<Vec<_>>();

        let mut consumed = 0;
        let input_ptr = input_data.as_ptr_range();
        (self.service_callback)(
            self.mouse_device.cast(),
            input_ptr.start,
            input_ptr.end,
            &mut consumed,
        );
    }
}

fn find_mouse_service_callback() -> anyhow::Result<MouseClassServiceCallbackFn> {
    let module_kdbclass = KModule::find_by_name(obfstr!("mouclass.sys"))?
        .with_context(|| anyhow!("failed to locate {} module", obfstr!("mouclass.sys")))?;

    let pattern =
        ByteSequencePattern::parse(obfstr!("48 8D 05 ? ? ? ? 48 89 44")).with_context(|| {
            obfstr!("Failed to compile MouseClassServiceCallback pattern").to_string()
        })?;

    NtOffsets::locate_function(
        &module_kdbclass,
        obfstr!("MouseClassServiceCallback"),
        &pattern,
        0x03,
        0x07,
    )
}

pub fn create_mouse_input() -> anyhow::Result<MouseInput> {
    let name = UNICODE_STRING::from_bytes(obfstr::wide!("\\Driver\\MouClass"));
    let mouse_device = Object::reference_by_name(&name, unsafe { *IoDriverObjectType })
        .map_err(|code| anyhow!("{} 0x{:X}", obfstr!("Object::reference_by_name"), code))?;
    let mouse_device = mouse_device.cast::<DRIVER_OBJECT>();

    /* To get all keyboard devices we could use kb_device.NextDevice. Currently we use the first one available. */
    let mouse_device = unsafe { mouse_device.DeviceObject.as_mut() };
    let mouse_device = match mouse_device {
        Some(device) => Object::reference(device as *mut _ as PVOID),
        None => anyhow::bail!("{}", obfstr!("no mouse device detected")),
    };

    let service_callback = find_mouse_service_callback()?;
    Ok(MouseInput {
        mouse_device,
        service_callback,
    })
}
