#![no_std]

extern crate alloc;

pub mod wrapper;

mod mem;
pub use mem::*;

pub fn init() -> anyhow::Result<()> {
    mem::initialize();
    wrapper::init()?;
    Ok(())
}
