use alloc::vec::Vec;
use core::mem::size_of_val;

use kapi::Process;
use valthrun_driver_shared::{
    requests::{
        MemoryAccessMode,
        RequestRead,
        ResponseRead,
    },
    IO_MAX_DEREF_COUNT,
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

    /// Resolved offsets while executing the read operation
    resolved_offsets: [u64; IO_MAX_DEREF_COUNT],

    /// Read offsets
    offsets: &'a [u64],

    /// Current resolved read offset index
    offset_index: usize,
}

#[allow(unused)]
fn read_memory_attached(ctx: &mut ReadContext) -> bool {
    let _attach_guard = ctx.process.attach();

    let mut current_address = ctx.offsets[0];
    while (ctx.offset_index + 1) < ctx.offsets.len() {
        let target = &mut ctx.resolved_offsets[ctx.offset_index];
        let target = unsafe {
            core::slice::from_raw_parts_mut(target as *mut u64 as *mut u8, size_of_val(target))
        };

        if !seh::probe_read(current_address, target.len(), 0x01) {
            return false;
        }
        if !seh::safe_copy(target, current_address) {
            return false;
        }

        // add the next offset
        current_address =
            ctx.resolved_offsets[ctx.offset_index].wrapping_add(ctx.offsets[ctx.offset_index + 1]);
        ctx.offset_index += 1;
    }

    if !seh::probe_read(current_address, ctx.read_buffer.len(), 0x01) {
        return false;
    }

    seh::safe_copy(ctx.read_buffer, current_address)
}

#[allow(unused)]
fn read_memory_mm(ctx: &mut ReadContext) -> bool {
    let current_process = Process::current();

    let mut current_address = ctx.offsets[0];
    while (ctx.offset_index + 1) < ctx.offsets.len() {
        let target = &mut ctx.resolved_offsets[ctx.offset_index];
        let target = unsafe {
            core::slice::from_raw_parts_mut(target as *mut u64 as *mut u8, size_of_val(target))
        };

        if !seh::probe_read(current_address, target.len(), 0x01) {
            return false;
        }

        let success = unsafe {
            let mut bytes_copied = 0usize;
            MmCopyVirtualMemory(
                ctx.process.eprocess(),
                current_address as *const c_void,
                current_process.eprocess(),
                target.as_mut_ptr() as *mut c_void,
                target.len(),
                KPROCESSOR_MODE::KernelMode,
                &mut bytes_copied,
            ) == STATUS_SUCCESS
        };
        if !success {
            return false;
        }

        // add the next offset
        current_address =
            ctx.resolved_offsets[ctx.offset_index].wrapping_add(ctx.offsets[ctx.offset_index + 1]);
        ctx.offset_index += 1;
    }

    if !seh::probe_read(current_address, ctx.read_buffer.len(), 0x01) {
        return false;
    }

    unsafe {
        let mut bytes_copied = 0usize;
        let status = MmCopyVirtualMemory(
            ctx.process.eprocess(),
            current_address as *const c_void,
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
    let mut current_address = ctx.offsets[0];
    while (ctx.offset_index + 1) < ctx.offsets.len() {
        let target = &mut ctx.resolved_offsets[ctx.offset_index];
        let target = unsafe {
            core::slice::from_raw_parts_mut(target as *mut u64 as *mut u8, size_of_val(target))
        };

        if pmem::read_process_memory(ctx.process, current_address, target).is_err() {
            return false;
        }

        // add the next offset
        current_address =
            ctx.resolved_offsets[ctx.offset_index].wrapping_add(ctx.offsets[ctx.offset_index + 1]);
        ctx.offset_index += 1;
    }

    pmem::read_process_memory(ctx.process, current_address, ctx.read_buffer).is_ok()
}

pub fn handler_read(req: &RequestRead, res: &mut ResponseRead) -> anyhow::Result<()> {
    if req.offset_count > IO_MAX_DEREF_COUNT || req.offset_count > req.offsets.len() {
        anyhow::bail!("offset count is not valid")
    }

    let out_buffer = unsafe { core::slice::from_raw_parts_mut(req.buffer, req.count) };
    if !seh::probe_write(
        out_buffer as *const _ as *const () as u64,
        out_buffer.len(),
        1,
    ) {
        anyhow::bail!("output buffer is not writeable")
    }

    let process = match Process::by_id(req.process_id) {
        Some(process) => process,
        None => {
            *res = ResponseRead::UnknownProcess;
            return Ok(());
        }
    };

    let mut read_buffer = Vec::with_capacity(req.count);
    read_buffer.resize(req.count, 0u8);

    let local_offsets = Vec::from(&req.offsets[0..req.offset_count]);
    let mut read_ctx = ReadContext {
        process: &process,

        read_buffer: read_buffer.as_mut_slice(),
        resolved_offsets: [0u64; IO_MAX_DEREF_COUNT],

        offsets: &local_offsets,
        offset_index: 0,
    };

    let read_result = match req.mode {
        MemoryAccessMode::AttachProcess => read_memory_attached(&mut read_ctx),

        // Do not expose them currently for normal valthrun users. CS2 does not check these.
        // MemoryAccessMode::CopyVirtualMemory => read_memory_mm(&mut read_ctx),
        // MemoryAccessMode::Physical => read_memory_physical(&mut read_ctx),
        #[allow(unreachable_patterns)]
        _ => {
            *res = ResponseRead::AccessModeUnavailable;
            return Ok(());
        }
    };

    if !read_result {
        *res = ResponseRead::InvalidAddress {
            resolved_offsets: read_ctx.resolved_offsets,
            resolved_offset_count: read_ctx.offset_index,
        };
        return Ok(());
    }

    /* Copy result to output */
    out_buffer.copy_from_slice(read_buffer.as_slice());
    *res = ResponseRead::Success;
    Ok(())
}
