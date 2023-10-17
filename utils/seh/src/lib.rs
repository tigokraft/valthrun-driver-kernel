#![no_std]

extern crate alloc;

use anyhow::anyhow;
use obfstr::obfstr;

pub mod wrapper;

mod mem;
pub use mem::*;

pub fn init() -> anyhow::Result<()> {
    wrapper::init()?;
    mem::init()
        .map_err(|err| anyhow!("{}: {:#}", obfstr!("failed to lookup SEH wrapped functions"), err))?;

    Ok(())
}