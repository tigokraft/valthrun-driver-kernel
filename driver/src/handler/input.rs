use core::mem::size_of;

use valthrun_driver_shared::{
    requests::{
        RequestKeyboardState,
        RequestMouseMove,
        ResponseKeyboardState,
        ResponseMouseMove,
    },
    KeyboardState,
    MouseState,
};

use crate::{
    KEYBOARD_INPUT,
    MOUSE_INPUT,
};

pub fn handler_mouse_move(
    req: &RequestMouseMove,
    _res: &mut ResponseMouseMove,
) -> anyhow::Result<()> {
    let input = unsafe { &*MOUSE_INPUT.get() };
    if let Some(input) = input {
        if !seh::probe_read(
            req.buffer as u64,
            req.state_count * size_of::<MouseState>(),
            1,
        ) {
            anyhow::bail!("invalid input buffer");
        }

        let state = unsafe { core::slice::from_raw_parts(req.buffer, req.state_count) };
        input.send_state(state);
    }

    Ok(())
}

pub fn handler_keyboard_state(
    req: &RequestKeyboardState,
    _res: &mut ResponseKeyboardState,
) -> anyhow::Result<()> {
    let input = unsafe { &*KEYBOARD_INPUT.get() };
    if let Some(input) = input {
        if !seh::probe_read(
            req.buffer as u64,
            req.state_count * size_of::<KeyboardState>(),
            1,
        ) {
            anyhow::bail!("invalid input buffer");
        }

        let state = unsafe { core::slice::from_raw_parts(req.buffer, req.state_count) };
        input.send_input(state);
    }

    Ok(())
}
