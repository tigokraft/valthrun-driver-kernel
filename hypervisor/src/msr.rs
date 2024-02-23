use bitfield_struct::bitfield;

#[bitfield(u64)]
pub struct Ia32FeatureControlMsr {
    pub locked: bool,
    pub enabled_smx: bool,
    pub enabled_vmxon: bool,

    #[bits(5)]
    _reserved2: usize,

    #[bits(7)]
    pub enabled_local_senter: u8,
    pub enabled_global_senter: bool,

    #[bits(16)]
    _reserved3a: u32,

    #[bits(32)]
    _reserved3b: u32,
}

#[bitfield(u64)]
pub struct Ia32VmxBasicMsr {
    #[bits(31)]
    pub revision_identifier: u32,
    _reserved1: bool,

    #[bits(12)]
    pub region_size: u32,
    pub region_clear: bool,

    #[bits(3)]
    _reserved2: u32,

    pub supported_ia64: bool,
    pub supported_dual_moniter: bool,
    #[bits(4)]
    pub memory_type: u8,
    pub vm_exit_report: bool,
    pub vmx_capability_hint: bool,

    #[bits(8)]
    _reserved3: u8,
}
