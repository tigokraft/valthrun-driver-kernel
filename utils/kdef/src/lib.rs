//! Low level kernel definitions which are not provided by the winapi
#![no_std]
#![allow(non_camel_case_types, non_snake_case)]

mod process;
use core::mem;

pub use process::*;

mod debug;
pub use debug::*;

mod object;
pub use object::*;

mod fast_mutex;
pub use fast_mutex::*;

mod ob;
pub use ob::*;

mod irp;
pub use irp::*;

mod timer;
pub use timer::*;

mod kbdclass;
pub use kbdclass::*;

mod mouclass;
pub use mouclass::*;
use winapi::km::wdm::PEPROCESS;

#[repr(C)]
pub struct _MDL {
    pub next: *mut _MDL,
    pub size: u16,
    pub mdl_flags: u16,
    pub process: PEPROCESS,
    pub mapped_system_va: *const (),
    pub start_va: *const (),
    pub byte_count: u32,
    pub byte_offset: u32,
}
const _: [(); 48] = [(); mem::size_of::<_MDL>()];
