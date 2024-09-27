#![no_std]
#![allow(dead_code)]
#![feature(sync_unsafe_cell)]
#![feature(allocator_api)]
#![feature(new_zeroed_alloc)]

extern crate alloc;

mod imports;
use alloc::string::ToString;

use anyhow::Context;

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
