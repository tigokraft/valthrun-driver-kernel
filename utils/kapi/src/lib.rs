#![no_std]
#![allow(dead_code)]
#![feature(sync_unsafe_cell)]
#![feature(new_uninit)]

extern crate alloc;

mod imports;
use alloc::string::ToString;

use anyhow::Context;
pub(crate) use imports::GLOBAL_IMPORTS;

mod process;
use obfstr::obfstr;
pub use process::*;

mod mdl;
pub use mdl::*;

mod string;
pub use string::*;

mod device;
pub use device::*;

mod status;
pub use status::*;

mod fast_mutex;
pub use fast_mutex::*;

mod irp;
pub use irp::*;

mod irql;
pub use irql::*;

mod object;
pub use object::*;

mod allocator;
pub use allocator::POOL_TAG;

pub mod thread;

mod event;
pub use event::*;

mod time;
pub use time::*;

mod timer;
pub use timer::*;

mod waitable;
pub use waitable::*;
use winapi::km::wdm::DRIVER_OBJECT;

pub fn initialize(driver: Option<&mut DRIVER_OBJECT>) -> anyhow::Result<()> {
    seh::init().with_context(|| obfstr!("seh").to_string())?;

    if let Some(driver) = driver {
        for function in driver.MajorFunction.iter_mut() {
            *function = Some(device_general_irp_handler);
        }
    }

    Ok(())
}
