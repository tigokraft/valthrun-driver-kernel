use bitfield_struct::bitfield;

use crate::ept::MemoryType;

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

#[bitfield(u64)]
pub struct Ia32MtrrPhysMaskRegister {
    /// [Bits 7:0] Specifies the memory type for the range.
    #[bits(8)]
    pub memory_type: MemoryType,

    #[bits(3)]
    _reserved1: u64,

    /// [Bit 11] Enables the register pair when set; disables register pair when clear.
    #[bits(1)]
    pub valid: bool,

    /// [Bits 47:12] Specifies a mask (24 bits if the maximum physical address size is 36 bits, 28 bits if the maximum physical
    /// address size is 40 bits). The mask determines the range of the region being mapped, according to the following
    /// relationships:
    /// - Address_Within_Range AND PhysMask = PhysBase AND PhysMask
    /// - This value is extended by 12 bits at the low end to form the mask value.
    /// - The width of the PhysMask field depends on the maximum physical address size supported by the processor.
    /// CPUID.80000008H reports the maximum physical address size supported by the processor. If CPUID.80000008H is not
    /// available, software may assume that the processor supports a 36-bit physical address size.
    ///
    /// @see Vol3A[11.11.3(Example Base and Mask Calculations)]
    #[bits(36)]
    pub page_frame_number: u64,

    #[bits(16)]
    _reserved2: u64,
}

#[bitfield(u64)]
pub struct Ia32MtrrCpabilitiesRegister {
    /// @brief VCNT (variable range registers count) field
    ///
    /// [Bits 7:0] Indicates the number of variable ranges implemented on the processor.
    #[bits(8)]
    pub variable_range_count: usize,

    /// @brief FIX (fixed range registers supported) flag
    ///
    /// [Bit 8] Fixed range MTRRs (MSR_IA32_MTRR_FIX64K_00000 through MSR_IA32_MTRR_FIX4K_0F8000) are supported when set; no fixed range
    /// registers are supported when clear.
    #[bits(1)]
    pub fixed_range_supported: bool,

    #[bits(1)]
    _reserved1: u64,

    /// @brief WC (write combining) flag
    ///
    /// [Bit 10] The write-combining (WC) memory type is supported when set; the WC type is not supported when clear.
    #[bits(1)]
    pub wc_supported: bool,

    /// @brief SMRR (System-Management Range Register) flag
    ///
    /// [Bit 11] The system-management range register (SMRR) interface is supported when bit 11 is set; the SMRR interface is
    /// not supported when clear.
    #[bits(1)]
    pub smrr_supported: bool,

    #[bits(52)]
    _reserved2: u64,
}

#[bitfield(u64)]
pub struct Ia32VmxEptVpidCapRegister {
    /// [Bit 0] When set to 1, the processor supports execute-only translations by EPT. This support allows software to
    /// configure EPT paging-structure entries in which bits 1:0 are clear (indicating that data accesses are not allowed) and
    /// bit 2 is set (indicating that instruction fetches are allowed).
    #[bits(1)]
    pub execute_only_pages: bool,

    #[bits(5)]
    _reserved1: u64,

    /// [Bit 6] Indicates support for a page-walk length of 4.
    #[bits(1)]
    pub page_walk_length4: bool,

    #[bits(1)]
    reserved2: u64,

    /// [Bit 8] When set to 1, the logical processor allows software to configure the EPT paging-structure memory type to be
    /// uncacheable (UC).
    ///
    /// @see Vol3C[24.6.11(Extended-Page-Table Pointer (EPTP))]
    #[bits(1)]
    pub memory_type_uncacheable: bool,

    #[bits(5)]
    reserved3: u64,

    /// [Bit 14] When set to 1, the logical processor allows software to configure the EPT paging-structure memory type to be write-back (WB).
    #[bits(1)]
    pub memory_type_write_back: bool,

    #[bits(1)]
    reserved4: u64,

    /// [Bit 16] When set to 1, the logical processor allows software to configure a EPT PDE to map a 2-Mbyte page (by setting bit 7 in the EPT PDE).
    #[bits(1)]
    pub pde2_mb_pages: bool,

    /// [Bit 17] When set to 1, the logical processor allows software to configure a EPT PDPTE to map a 1-Gbyte page (by setting bit 7 in the EPT PDPTE).
    #[bits(1)]
    pub pdpte1_gb_pages: bool,

    #[bits(2)]
    reserved5: u64,

    /// [Bit 20] If bit 20 is read as 1, the INVEPT instruction is supported.
    ///
    /// @see Vol3C[30(VMX INSTRUCTION REFERENCE)]
    /// @see Vol3C[28.3.3.1(Operations that Invalidate Cached Mappings)]
    #[bits(1)]
    pub invept: bool,

    /// [Bit 21] When set to 1, accessed and dirty flags for EPT are supported.
    /// @see Vol3C[28.2.4(Accessed and Dirty Flags for EPT)]
    #[bits(1)]
    pub ept_accessed_and_dirty_flags: bool,

    ///
    /// [Bit 22] When set to 1, the processor reports advanced VM-exit information for EPT violations. This reporting is done
    /// only if this bit is read as 1.
    ///
    /// @see Vol3C[27.2.1(Basic VM-Exit Information)]
    #[bits(1)]
    pub advanced_vmexit_ept_violations_information: bool,

    #[bits(2)]
    reserved6: u64,

    /// [Bit 25] When set to 1, the single-context INVEPT type is supported.
    ///
    /// @see Vol3C[30(VMX INSTRUCTION REFERENCE)]
    /// @see Vol3C[28.3.3.1(Operations that Invalidate Cached Mappings)]
    #[bits(1)]
    pub invept_single_context: bool,

    /// [Bit 26] When set to 1, the all-context INVEPT type is supported.
    ///
    /// @see Vol3C[30(VMX INSTRUCTION REFERENCE)]
    /// @see Vol3C[28.3.3.1(Operations that Invalidate Cached Mappings)]
    #[bits(1)]
    pub invept_all_contexts: bool,

    #[bits(5)]
    pub reserved7: u64,

    /// [Bit 32] When set to 1, the INVVPID instruction is supported.
    #[bits(1)]
    pub invvpid: bool,

    #[bits(7)]
    reserved8: u64,

    /// [bit 40] when set to 1, the individual-address invvpid type is supported.
    #[bits(1)]
    pub invvpid_individual_address: bool,

    /// [bit 41] when set to 1, the single-context invvpid type is supported.
    #[bits(1)]
    pub invvpid_single_context: bool,

    /// [bit 42] when set to 1, the all-context invvpid type is supported.
    #[bits(1)]
    pub invvpid_all_contexts: bool,

    /// [bit 43] when set to 1, the single-context-retaining-globals invvpid type is supported.
    #[bits(1)]
    pub invvpid_single_context_retain_globals: bool,

    #[bits(20)]
    reserved9: u64,
}
