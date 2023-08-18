use core::mem::size_of_val;

use alloc::vec::Vec;
use valthrun_driver_shared::{requests::{RequestRead, ResponseRead}, IO_MAX_DEREF_COUNT};

use crate::kapi::{Process, mem};

fn read_memory(target: &mut [ u8 ], offsets: &[ u64 ], resolved_offsets: &mut [ u64 ], offset_index: &mut usize) -> bool {
    let mut current_address = offsets[0];
    while (*offset_index + 1) < offsets.len() {
        let target = unsafe {
            let target = resolved_offsets.as_mut_ptr()
                .offset(*offset_index as isize)
                .cast::<u8>();

            core::slice::from_raw_parts_mut(target, size_of_val(&current_address))
        };
        
        if !mem::safe_copy(target, current_address) {
            return false;
        }
        current_address = resolved_offsets[*offset_index].wrapping_add(offsets[*offset_index + 1]); // add the next offset
        *offset_index += 1;
    }

    mem::safe_copy(target, current_address)
}

pub fn handler_read(req: &RequestRead, res: &mut ResponseRead) -> anyhow::Result<()> {
    if req.offset_count > IO_MAX_DEREF_COUNT || req.offset_count > req.offsets.len() {
        anyhow::bail!("offset count is not valid")
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
    let mut resolved_offsets = [0u64; IO_MAX_DEREF_COUNT];
    let mut offset_index = 0usize;

    let read_result = {
        let _attach_guard = process.attach();
        read_memory(
            read_buffer.as_mut_slice(), 
            local_offsets.as_slice(), 
            &mut resolved_offsets, &mut offset_index
        )
    };

    if !read_result {
        *res = ResponseRead::InvalidAddress { resolved_offsets, resolved_offset_count: offset_index  };
        return Ok(());
    }

    /* Copy result to output */
    let out_buffer = unsafe {
        core::slice::from_raw_parts_mut(req.buffer, req.count)
    };
    out_buffer.copy_from_slice(read_buffer.as_slice());
    *res = ResponseRead::Success;
    Ok(())
}