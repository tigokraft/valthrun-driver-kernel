#![no_std]

extern crate alloc;

mod def;
mod imports;
pub use imports::resolve_import;

mod module;
pub use module::{
    KModule,
    KModuleSection,
};
