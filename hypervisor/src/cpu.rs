use alloc::string::{
    String,
    ToString,
};

use x86::cpuid::cpuid;

pub fn name() -> String {
    let cpuid = cpuid!(0);
    let mut name_buffer = [0u8; 12];
    name_buffer[0..4].copy_from_slice(&cpuid.ebx.to_le_bytes());
    name_buffer[4..8].copy_from_slice(&cpuid.edx.to_le_bytes());
    name_buffer[8..12].copy_from_slice(&cpuid.ecx.to_le_bytes());

    String::from_utf8_lossy(&name_buffer).to_string()
}

pub fn hypervisor_id() -> String {
    let cpuid = cpuid!(0x40000001);
    let mut name_buffer = [0u8; 4];
    name_buffer[0..4].copy_from_slice(&cpuid.eax.to_le_bytes());
    String::from_utf8_lossy(&name_buffer).to_string()
}
