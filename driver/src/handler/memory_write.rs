use kapi::Process;
use obfstr::obfstr;
use vtd_protocol::{
    command::DriverCommandMemoryWrite,
    types::MemoryAccessResult,
};
use winapi::{
    km::wdm::KPROCESSOR_MODE,
    shared::{
        ntdef::PVOID,
        ntstatus::STATUS_SUCCESS,
    },
};

use crate::imports::MmCopyVirtualMemory;

struct WriteContext<'a> {
    /// Target process where we want to read the data from
    process: &'a Process,

    /// Buffer to copy the data from.
    /// ATTENTION: This is the user supplied buffer and **not** a kernel buffer!
    buffer: &'a [u8],

    /// Target address
    address: u64,
}

fn write_memory_attached(ctx: &WriteContext) -> bool {
    /* copy into kernel memory */
    let buffer = ctx.buffer.to_vec();

    let _attach_guard = ctx.process.attach();
    let target_buffer = unsafe {
        if !seh::probe_write(ctx.address, buffer.len(), 0x01) {
            return false;
        }

        core::slice::from_raw_parts_mut(ctx.address as *mut u8, buffer.len())
    };

    seh::safe_copy(target_buffer, buffer.as_ptr() as u64)
}

fn write_memory_mm(ctx: &WriteContext) -> bool {
    let current_process = Process::current();

    unsafe {
        let mut bytes_copied = 0usize;
        let status = MmCopyVirtualMemory(
            current_process.eprocess(),
            ctx.buffer.as_ptr() as PVOID,
            ctx.process.eprocess(),
            ctx.address as PVOID,
            ctx.buffer.len(),
            KPROCESSOR_MODE::KernelMode,
            &mut bytes_copied,
        );

        status == STATUS_SUCCESS
    }
}

pub fn handler_write(command: &mut DriverCommandMemoryWrite) -> anyhow::Result<()> {
    let buffer = unsafe {
        if !seh::probe_read(command.buffer as u64, command.count, 0x01) {
            anyhow::bail!("{}", obfstr!("output buffer is not writeable"))
        }

        core::slice::from_raw_parts(command.buffer, command.count)
    };

    let process = match Process::by_id(command.process_id as i32) {
        Some(process) => process,
        None => {
            command.result = MemoryAccessResult::ProcessUnknown;
            return Ok(());
        }
    };

    let ctx = WriteContext {
        process: &process,
        address: command.address as u64,
        buffer,
    };

    let success = write_memory_attached(&ctx);
    // let success = write_memory_mm(&ctx);

    if !success {
        command.result = MemoryAccessResult::PartialSuccess { bytes_copied: 0 };
        return Ok(());
    }

    command.result = MemoryAccessResult::Success;
    Ok(())
}
