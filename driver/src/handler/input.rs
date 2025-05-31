use core::mem::size_of;

use vtd_protocol::command::{
    DriverCommandInputKeyboard,
    DriverCommandInputMouse,
    KeyboardState,
    MouseState,
};

use crate::{
    KEYBOARD_INPUT,
    MOUSE_INPUT,
};

pub fn handler_mouse_move(command: &mut DriverCommandInputMouse) -> anyhow::Result<()> {
    let input = unsafe { &*MOUSE_INPUT.get() };
    if let Some(input) = input {
        if !seh::probe_read(
            command.buffer as u64,
            command.state_count * size_of::<MouseState>(),
            1,
        ) {
            anyhow::bail!("invalid input buffer");
        }

        let state = unsafe { core::slice::from_raw_parts(command.buffer, command.state_count) };
        input.send_state(state);
    }

    Ok(())
}

pub fn handler_keyboard_state(command: &mut DriverCommandInputKeyboard) -> anyhow::Result<()> {
    let input = unsafe { &*KEYBOARD_INPUT.get() };
    if let Some(input) = input {
        if !seh::probe_read(
            command.buffer as u64,
            command.state_count * size_of::<KeyboardState>(),
            1,
        ) {
            anyhow::bail!("invalid input buffer");
        }

        let state = unsafe { core::slice::from_raw_parts(command.buffer, command.state_count) };
        input.send_input(state);
    }

    Ok(())
}
