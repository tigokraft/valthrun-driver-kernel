use core::{
    mem::size_of,
    str,
};

use kapi::Process;
use obfstr::obfstr;
use vtd_protocol::{
    command::{
        DriverCommandProcessList,
        DriverCommandProcessModules,
    },
    types::{
        ProcessId,
        ProcessInfo,
        ProcessModuleInfo,
    },
};

use crate::{
    offsets::get_nt_offsets,
    util::kprocess,
};

pub fn handler_get_processes(command: &mut DriverCommandProcessList) -> anyhow::Result<()> {
    let buffer = unsafe {
        if !seh::probe_write(
            command.buffer as u64,
            command.buffer_capacity * size_of::<ProcessInfo>(),
            0x01,
        ) {
            anyhow::bail!("{}", obfstr!("response buffer not writeable"));
        }

        core::slice::from_raw_parts_mut(command.buffer, command.buffer_capacity)
    };

    command.process_count = 0;
    kprocess::iter(|process| {
        {
            #[allow(non_snake_case)]
            let EPROCESS_ThreadListHead = get_nt_offsets().EPROCESS_ThreadListHead;

            let active_threads = unsafe {
                process.eprocess()
                    /* The ActiveThreads comes after the thread list head. Thread list head has a size of 0x10. */
                    .byte_offset(EPROCESS_ThreadListHead as isize + 0x10)
                    .cast::<u32>()
                    .read_volatile()
            };

            if active_threads == 0 {
                /* Process terminated / not running */
                return;
            }
        }
        if let Some(output) = buffer.get_mut(command.process_count) {
            output.process_id = process.get_id() as ProcessId;
            output.directory_table_base = process.get_directory_table_base();

            {
                let image_base_name = process.get_image_file_name().unwrap_or_default();
                let copy_length = image_base_name.len().min(output.image_base_name.len());
                output.image_base_name[0..copy_length]
                    .copy_from_slice(&image_base_name.as_bytes()[0..copy_length]);

                if copy_length < output.image_base_name.len() - 1 {
                    output.image_base_name[copy_length] = 0x00;
                }
            }
        }

        command.process_count += 1;
    });

    Ok(())
}

pub fn handler_get_modules(command: &mut DriverCommandProcessModules) -> anyhow::Result<()> {
    let buffer = unsafe {
        if !seh::probe_write(
            command.buffer as u64,
            command.buffer_capacity * size_of::<ProcessModuleInfo>(),
            0x01,
        ) {
            anyhow::bail!("{}", obfstr!("response buffer not writeable"));
        }

        core::slice::from_raw_parts_mut(command.buffer, command.buffer_capacity)
    };

    let Some(process) = Process::by_id(command.process_id as i32) else {
        command.process_unknown = true;
        return Ok(());
    };

    let modules = {
        let attached_process = process.attach();
        attached_process.get_modules()
    };

    command.process_unknown = false;
    for module in &modules {
        if let Some(output) = buffer.get_mut(command.module_count) {
            output.base_dll_name.copy_from_slice(&module.base_dll_name);
            output.base_address = module.base_address as u64;
            output.module_size = module.module_size as u64;
        }

        command.module_count += 1;
    }

    Ok(())
}
