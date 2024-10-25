use alloc::vec;
use core::{
    mem::size_of,
    str,
};

use kapi::Process;
use obfstr::obfstr;
use valthrun_driver_shared::{
    requests::{
        ProcessFilter,
        RequestProcessModules,
        ResponseProcessModules,
    },
    ModuleInfo,
    ProcessModuleInfo,
};

use crate::util::kprocess;

pub fn handler_get_modules(
    req: &RequestProcessModules,
    res: &mut ResponseProcessModules,
) -> anyhow::Result<()> {
    handler_get_modules_internal(req, res, false)
}

pub fn handler_get_modules_internal(
    req: &RequestProcessModules,
    res: &mut ResponseProcessModules,
    kernel_filter: bool,
) -> anyhow::Result<()> {
    let module_buffer = unsafe {
        if !seh::probe_write(
            req.module_buffer as u64,
            req.module_buffer_length * size_of::<ModuleInfo>(),
            0x01,
        ) {
            anyhow::bail!("{}", obfstr!("response buffer not writeable"));
        }

        core::slice::from_raw_parts_mut(req.module_buffer, req.module_buffer_length)
    };

    let process_candidates = match req.filter {
        ProcessFilter::Id { id } => Process::by_id(id).map(|p| vec![p]).unwrap_or_default(),
        ProcessFilter::Name { name, name_length } => {
            let name = unsafe {
                if !kernel_filter && !seh::probe_read(name as u64, name_length, 0x01) {
                    anyhow::bail!("{}", obfstr!("name buffer not readable"));
                }

                core::slice::from_raw_parts(name, name_length)
            };
            let target_name =
                str::from_utf8(name).map_err(|_| anyhow::anyhow!("invalid name (not utf-8)"))?;
            kprocess::find_processes_by_name(target_name)?
        }
    };

    let process = match process_candidates.len() {
        0 => {
            *res = ResponseProcessModules::NoProcess;
            return Ok(());
        }
        1 => process_candidates.first().unwrap(),
        count => {
            *res = ResponseProcessModules::UbiquitousProcesses(count);
            return Ok(());
        }
    };

    let process_id = process.get_id();
    log::trace!(
        "Found process id {}. PEP at {:X}",
        process_id,
        process.eprocess() as u64
    );

    let modules = {
        let attached_process = process.attach();
        attached_process.get_modules()
    };

    if modules.len() > module_buffer.len() {
        *res = ResponseProcessModules::BufferTooSmall {
            expected: modules.len(),
        };
        return Ok(());
    }

    module_buffer[0..modules.len()].copy_from_slice(&modules);

    let mut module_info: ProcessModuleInfo = Default::default();
    module_info.process_id = process_id;
    module_info.module_count = modules.len();
    *res = ResponseProcessModules::Success(module_info);
    Ok(())
}
