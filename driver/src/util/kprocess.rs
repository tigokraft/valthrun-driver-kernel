use alloc::vec::Vec;
use core::ffi::CStr;

use kapi::Process;

use crate::{
    imports::GLOBAL_IMPORTS,
    offsets::get_nt_offsets,
};

pub fn find_processes_by_name(target_name: &str) -> anyhow::Result<Vec<Process>> {
    let imports = GLOBAL_IMPORTS.unwrap();

    #[allow(non_snake_case)]
    let PsGetNextProcess = get_nt_offsets().PsGetNextProcess;

    #[allow(non_snake_case)]
    let EPROCESS_ThreadListHead = get_nt_offsets().EPROCESS_ThreadListHead;

    let mut cs2_candidates = Vec::with_capacity(8);

    let mut current_peprocess = core::ptr::null_mut();
    loop {
        current_peprocess = unsafe { PsGetNextProcess(current_peprocess) };
        if current_peprocess.is_null() {
            break;
        }

        let image_file_name = unsafe {
            CStr::from_ptr((imports.PsGetProcessImageFileName)(current_peprocess))
                .to_str()
                .ok()
        };

        if image_file_name != Some(target_name) {
            continue;
        }

        let active_threads = unsafe {
            current_peprocess
                /* The ActiveThreads comes after the thread list head. Thread list head has a size of 0x10. */
                .byte_offset(EPROCESS_ThreadListHead as isize + 0x10)
                .cast::<u32>()
                .read_volatile()
        };

        log::trace!(
            "{} matched {:X}: {:?} ({})",
            target_name,
            current_peprocess as u64,
            image_file_name,
            active_threads
        );
        if active_threads == 0 {
            /* Process terminated / not running */
            continue;
        }

        cs2_candidates.push(Process::from_raw(current_peprocess, false));
    }

    Ok(cs2_candidates)
}
