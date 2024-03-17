use alloc::string::{
    String,
    ToString,
};

use x86::cpuid::{
    cpuid,
    CpuId,
};

pub fn name() -> String {
    CpuId::new()
        .get_vendor_info()
        .map(|info| info.as_str().to_string())
        .unwrap_or_default()
}

pub fn hypervisor_id() -> String {
    let cpuid = cpuid!(0x40000001);
    let mut name_buffer = [0u8; 4];
    name_buffer[0..4].copy_from_slice(&cpuid.eax.to_le_bytes());
    String::from_utf8_lossy(&name_buffer).to_string()
}
