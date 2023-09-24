use core::mem::size_of;

use winapi::{shared::ntdef::{PVOID, PCVOID, NTSTATUS}, ctypes::c_void};

use crate::{kapi::{Process, NTStatusEx}, winver::os_info};

fn get_process_cr3(process: &Process) -> u64 {
    let dtb = process.get_directory_table_base();
    if dtb != 0 {
        return dtb;
    }

    let udtb_offset = match os_info().dwBuildNumber {
        17134 => 0x0278, // WINDOWS_1803
        17763 => 0x0278, // WINDOWS_1803
        18362 => 0x0280, // WINDOWS_1903
        18363 => 0x0280, // WINDOWS_1909
        19041 => 0x0388, // WINDOWS_2004
        19569 => 0x0388, // WINDOWS_20H2
        20180 => 0x0388, // WINDOWS_21H1
        _ => 0x0388, // FIXME: Don't hardcode this offset!
    };

    // Getting the UserDirectoryTableBase.
    // Windows equivilant to https://en.wikipedia.org/wiki/Kernel_page-table_isolation
    unsafe {
        *process.eprocess()
            .byte_offset(udtb_offset)
            .cast::<u64>()
    }
}

#[allow(unused)]
const MM_COPY_MEMORY_PHYSICAL: u32 = 0x01;

#[allow(unused)]
const MM_COPY_MEMORY_VIRTUAL: u32 = 0x02;

extern "system" {
    fn MmCopyMemory(
        TargetAddress: PVOID,
        SourceAddress: PCVOID,
        NumberOfBytes: usize,
        Flags: u32,
        NumberOfBytesTransferred: *mut usize
    ) -> NTSTATUS;
}

fn read_physical(address: u64, buffer: &mut [u8]) -> Result<(), (NTSTATUS, usize)> {
    let mut bytes_copied = 0;
    let status = unsafe {
        MmCopyMemory(
            buffer.as_mut_ptr() as *mut c_void, 
            address as *const () as *const c_void,
            buffer.len() as usize,
            MM_COPY_MEMORY_PHYSICAL,
            &mut bytes_copied
        )
    };

    if status.is_ok() {
        Ok(())
    } else {
        log::trace!("read_physical failed at {:X}: {:X}. Read {} bytes.", address, status, bytes_copied);
        Err((status, bytes_copied))
    }
}

fn read_physical_u64(address: u64) -> Result<u64, NTSTATUS> {
    let mut buffer = [0u8; size_of::<u64>()];

    match read_physical(address, &mut buffer) {
        Ok(_) => {},
        Err((status, _)) => return Err(status)
    };

    Ok(u64::from_le_bytes(buffer))
}

const PAGE_OFFSET_SIZE: u64 = 12;
const PMASK: u64 = (!0xF << 8) & 0xfffffffff;
pub fn translate_linear_address(directory_table_base: u64, virtual_address: u64) -> Option<u64> {
    let directory_table_base = directory_table_base & !0xF;

    let page_offset = virtual_address & !(!0 << PAGE_OFFSET_SIZE);
    let pte = (virtual_address >> 12) & 0x1FF;
    let pt = (virtual_address >> 21) & 0x1FF;
    let pd = (virtual_address >> 30) & 0x1FF;
    let pdp = (virtual_address >> 39) & 0x1FF;

    let pdp_address = read_physical_u64(directory_table_base + pdp * 0x08).ok()?;
    if !pdp_address & 0x01 > 0 {
        return None;
    }

    let pd_value = read_physical_u64((pdp_address & PMASK) + pd * 0x08).ok()?;
    if !pd_value & 0x01 > 0 {
        return None;
    }

    if pd_value & 0x80 > 0 {
        /* 1GB large page, use pde's 12-34 bits */
        return Some((pd_value & (!0 << 42 >> 12)) + (virtual_address & !(!0 << 30)));  
    }

    let pt_value = read_physical_u64((pd_value & PMASK) + pt * 0x08).ok()?;
    if !pt_value & 0x01 > 0 {
        return None;
    }
    
    if pt_value & 0x80 > 0 {
        /* 2MB large page */
        return Some((pt_value & PMASK) + (virtual_address & !(!0 << 21))); 
    }

    let pte_value = read_physical_u64((pt_value & PMASK) + pte * 0x08).ok()? & PMASK;
    if pte_value == 0 {
        return None;
    }

    return Some(pte_value + page_offset);
}

const PAGE_SIZE: u64 = 1 << 12;
pub fn read_process_memory(process: &Process, virtual_address: u64, buffer: &mut [u8]) -> Result<(), usize> {
    let directory_table_base = get_process_cr3(process);

    let start_offset = (virtual_address & (PAGE_SIZE - 1)) as usize;
    let mut bytes_read = 0usize;
    let (remainding_buffer, remainding_chunk_offset) = if start_offset == 0 {
        (buffer, 0)
    } else {
        let start_physical_address = match translate_linear_address(directory_table_base, virtual_address) {
            Some(address) => address,
            None => return Err(bytes_read),
        };

        let start_bytes = (PAGE_SIZE as usize - start_offset).min(buffer.len());
        if let Err((_, bytes)) = read_physical(start_physical_address, &mut buffer[0..start_bytes]) {
            bytes_read += bytes;
            return Err(bytes_read);
        }

        bytes_read += start_bytes;
        (&mut buffer[start_bytes..], 1)
    };

    for (chunk_index, chunk_buffer) in remainding_buffer.chunks_mut(PAGE_SIZE as usize).enumerate() {
        let chunk_virtual_address = (virtual_address & !(PAGE_SIZE - 1)) + (chunk_index as u64 + remainding_chunk_offset) * PAGE_SIZE;
        let chunk_physical_address = match translate_linear_address(directory_table_base, chunk_virtual_address) {
            Some(address) => address,
            None => return Err(bytes_read),
        };

        if let Err((_, bytes)) = read_physical(chunk_physical_address, chunk_buffer) {
            bytes_read += bytes;
            return Err(bytes_read);
        }

        bytes_read += chunk_buffer.len();
    }

    Ok(())
}