use core::mem::size_of;

use obfstr::obfstr;
use valthrun_driver_shared::{
    requests::{
        RequestCSModule,
        ResponseCsModule,
    },
    ModuleInfo,
    ProcessModuleInfo,
};

use crate::util::kprocess;

pub fn handler_get_modules(
    req: &RequestCSModule,
    res: &mut ResponseCsModule,
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

    log::debug!("{}", obfstr!("Searching for CS2 process."));
    let cs2_process_candidates = kprocess::find_processes_by_name(obfstr!("cs2.exe"))?;
    let process = match cs2_process_candidates.len() {
        0 => {
            *res = ResponseCsModule::NoProcess;
            return Ok(());
        }
        1 => cs2_process_candidates.first().unwrap(),
        count => {
            *res = ResponseCsModule::UbiquitousProcesses(count);
            return Ok(());
        }
    };

    let process_id = process.get_id();
    log::trace!(
        "{} process id {}. PEP at {:X}",
        obfstr!("CS2"),
        process_id,
        process.eprocess() as u64
    );

    let modules = {
        let attached_process = process.attach();
        attached_process.get_modules()
    };

    if modules.len() > module_buffer.len() {
        *res = ResponseCsModule::BufferTooSmall {
            expected: modules.len(),
        };
        return Ok(());
    }

    module_buffer[0..modules.len()].copy_from_slice(&modules);

    let mut module_info: ProcessModuleInfo = Default::default();
    module_info.process_id = process_id;
    module_info.module_count = modules.len();
    *res = ResponseCsModule::Success(module_info);
    Ok(())
}
