#![no_std]
#![feature(sync_unsafe_cell)]
#![feature(error_in_core)]
#![feature(pointer_byte_offsets)]

extern crate alloc;

pub mod ll;
pub use obfstr::obfstr;
pub use paste::paste;

pub mod provider;

mod error;
pub use error::*;

mod table;
pub use table::*;

mod r#macro;
// pub use r#macro::dynamic_import_table;

pub fn initialize() {
    ll::init();
}
