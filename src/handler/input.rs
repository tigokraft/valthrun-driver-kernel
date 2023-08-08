use core::mem::size_of;

use valthrun_driver_shared::{requests::{RequestMouseMove, ResponseMouseMove, RequestKeyboardState, ResponseKeyboardState}, MouseState, KeyboardState};

use crate::{MOUSE_INPUT, KEYBOARD_INPUT, kdef::ProbeForRead, kapi};


pub fn handler_mouse_move(req: &RequestMouseMove, _res: &mut ResponseMouseMove) -> anyhow::Result<()> {
    let input = unsafe { &*MOUSE_INPUT.get() };
    if let Some(input) = input {
        let state_valid = kapi::try_seh(|| {
            unsafe { ProbeForRead(req.buffer as *const (), req.state_count * size_of::<MouseState>(), 1); }
        }).is_ok();
        if !state_valid {
            anyhow::bail!("invalid input buffer");
        }

        let state = unsafe {
            core::slice::from_raw_parts(req.buffer, req.state_count)
        };
        input.send_state(state);
    }
    
    Ok(())
}

pub fn handler_keyboard_state(req: &RequestKeyboardState, _res: &mut ResponseKeyboardState) -> anyhow::Result<()> {
    let input = unsafe { &*KEYBOARD_INPUT.get() };
    if let Some(input) = input {
        let state_valid = kapi::try_seh(|| {
            unsafe { ProbeForRead(req.buffer as *const (), req.state_count * size_of::<KeyboardState>(), 1); }
        }).is_ok();
        if !state_valid {
            anyhow::bail!("invalid input buffer");
        }

        let state = unsafe { core::slice::from_raw_parts(req.buffer, req.state_count) };
        input.send_input(state);
    }
    
    Ok(())
}