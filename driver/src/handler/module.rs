use alloc::vec;
use core::{
    mem::size_of,
    str,
};

use kapi::Process;
use obfstr::obfstr;
use valthrun_driver_protocol::{
    command::{
        DriverCommandProcessModules,
        ProcessModulesResult,
    },
    types::{
        ProcessFilter,
        ProcessModuleInfo,
    },
};

use crate::util::kprocess;

pub fn handler_get_modules(command: &mut DriverCommandProcessModules) -> anyhow::Result<()> {
    let module_buffer = unsafe {
        if !seh::probe_write(
            command.module_buffer as u64,
            command.module_buffer_length * size_of::<ProcessModuleInfo>(),
            0x01,
        ) {
            anyhow::bail!("{}", obfstr!("response buffer not writeable"));
        }

        core::slice::from_raw_parts_mut(command.module_buffer, command.module_buffer_length)
    };

    let process_candidates = match command.target_process {
        ProcessFilter::None => {
            command.result = ProcessModulesResult::ProcessUnknown;
            return Ok(());
        }
        ProcessFilter::Id { id } => {
            Process::by_id(id as i32)
                .map(|p| vec![p])
                .unwrap_or_default()
        }
        ProcessFilter::ImageBaseName { name, name_length } => {
            let name = unsafe {
                if !seh::probe_read(name as u64, name_length, 0x01) {
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
            command.result = ProcessModulesResult::ProcessUnknown;
            return Ok(());
        }
        1 => process_candidates.first().unwrap(),
        _count => {
            command.result = ProcessModulesResult::ProcessUbiquitous;
            return Ok(());
        }
    };

    command.process_id = process.get_id() as u32;
    log::trace!(
        "Found process id {}. PEP at {:X}",
        command.process_id,
        process.eprocess() as u64
    );

    let modules = {
        let attached_process = process.attach();
        attached_process.get_modules()
    };

    command.module_count = modules.len();
    if modules.len() > module_buffer.len() {
        command.result = ProcessModulesResult::BufferTooSmall;
        return Ok(());
    }

    for index in 0..modules.len() {
        let output = &mut module_buffer[index];
        let input = &modules[index];

        output.base_dll_name.copy_from_slice(&input.base_dll_name);
        output.base_address = input.base_address as u64;
        output.module_size = input.module_size as u64;
    }

    command.result = ProcessModulesResult::Success;
    Ok(())
}
