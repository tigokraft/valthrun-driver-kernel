#![no_std]
#![feature(error_in_core)]
#![feature(sync_unsafe_cell)]

mod r#macro;
pub mod provider;
pub mod utils;

mod error;
pub use error::*;

mod table;
/* re-exports for the dynamic_import_table macro */
pub use obfstr::obfstr;
pub use paste::paste;
pub use table::*;
