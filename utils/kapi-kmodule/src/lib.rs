#![no_std]
#![feature(pointer_byte_offsets)]

extern crate alloc;

mod module;
pub use module::*;

mod imports;

mod imports_module;
pub use imports_module::*;

pub use valthrun_driver_shared::{
    SearchPattern,
    BytePattern,
    ByteSequencePattern,
};