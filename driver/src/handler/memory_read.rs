use alloc::vec::Vec;

use kapi::Process;
use valthrun_driver_protocol::{
    command::DriverCommandMemoryRead,
    types::MemoryAccessResult,
};
use winapi::{
    ctypes::c_void,
    km::wdm::KPROCESSOR_MODE,
    shared::ntstatus::STATUS_SUCCESS,
};

use crate::{
    imports::MmCopyVirtualMemory,
    pmem,
};

struct ReadContext<'a> {
    /// Target process where we want to read the data from
    process: &'a Process,

    /// Target non paged kernel space buffer we copy the data into
    read_buffer: &'a mut [u8],

    /// Address of the read operation
    target_address: u64,
}

#[allow(unused)]
fn read_memory_attached(ctx: &mut ReadContext) -> bool {
    let _attach_guard = ctx.process.attach();

    if !seh::probe_read(ctx.target_address, ctx.read_buffer.len(), 0x01) {
        return false;
    }

    seh::safe_copy(ctx.read_buffer, ctx.target_address)
}

#[allow(unused)]
fn read_memory_mm(ctx: &mut ReadContext) -> bool {
    let current_process = Process::current();

    if !seh::probe_read(ctx.target_address, ctx.read_buffer.len(), 0x01) {
        return false;
    }

    unsafe {
        let mut bytes_copied = 0usize;
        let status = MmCopyVirtualMemory(
            ctx.process.eprocess(),
            ctx.target_address as *const c_void,
            current_process.eprocess(),
            ctx.read_buffer.as_mut_ptr() as *mut c_void,
            ctx.read_buffer.len(),
            KPROCESSOR_MODE::KernelMode,
            &mut bytes_copied,
        );

        status == STATUS_SUCCESS
    }
}

// Side note:
// We may not need to use read_process_memory if we just set the current cr3 to the target processes
// cr3 value and then do normal buisness.
#[allow(unused)]
fn read_memory_physical(ctx: &mut ReadContext) -> bool {
    pmem::read_process_memory(ctx.process, ctx.target_address, ctx.read_buffer).is_ok()
}

pub fn handler_read(command: &mut DriverCommandMemoryRead) -> anyhow::Result<()> {
    let out_buffer = unsafe { core::slice::from_raw_parts_mut(command.buffer, command.count) };
    if !seh::probe_write(
        out_buffer as *const _ as *const () as u64,
        out_buffer.len(),
        1,
    ) {
        anyhow::bail!("output buffer is not writeable")
    }

    let process = match Process::by_id(command.process_id as i32) {
        Some(process) => process,
        None => {
            command.result = MemoryAccessResult::ProcessUnknown;
            return Ok(());
        }
    };

    let mut read_buffer = Vec::with_capacity(command.count);
    read_buffer.resize(command.count, 0u8);

    let mut read_ctx = ReadContext {
        process: &process,

        read_buffer: read_buffer.as_mut_slice(),
        target_address: command.address,
    };

    let read_result = read_memory_attached(&mut read_ctx);
    // let read_result = read_memory_mm(&mut read_ctx);
    // let read_result = read_memory_physical(&mut read_ctx);

    if !read_result {
        command.result = MemoryAccessResult::PartialSuccess { bytes_copied: 0 };
        return Ok(());
    }

    /* Copy result to output */
    out_buffer.copy_from_slice(read_buffer.as_slice());
    command.result = MemoryAccessResult::Success;
    Ok(())
}
