use alloc::{
    boxed::Box,
    string::ToString,
};

use valthrun_driver_shared::requests::{
    ProcessFilter,
    RequestCSModule,
    RequestProcessModules,
    ResponseProcessModules,
};

use super::handler_get_modules_internal;
use crate::handler::handler_get_modules;

pub fn handler_get_cs2_modules(
    req: &RequestCSModule,
    res: &mut ResponseProcessModules,
) -> anyhow::Result<()> {
    let process_name = "cs2.exe";
    handler_get_modules_internal(
        &RequestProcessModules {
            filter: ProcessFilter::Name {
                name: process_name.as_ptr(),
                name_length: process_name.len(),
            },
            module_buffer: req.module_buffer,
            module_buffer_length: req.module_buffer_length,
        },
        res,
        true,
    )
}
