use alloc::boxed::Box;

use anyhow::Result;
use bitfield_struct::bitfield;
use kalloc::ContiguousMemoryAllocator;
use x86::msr::{
    rdmsr,
    IA32_MTRRCAP,
    IA32_MTRR_PHYSBASE0,
    IA32_MTRR_PHYSMASK0,
};

use crate::{
    mem::MemoryAddressEx,
    msr::{
        Ia32MtrrCpabilitiesRegister,
        Ia32MtrrPhysMaskRegister,
    },
};

/// A custom enum
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum MemoryType {
    Uncacheable = 0,
    WriteCombinding = 1,
    WriteThrough = 4,
    WriteProtect = 5,
    WriteBack = 6,
}

impl Default for MemoryType {
    fn default() -> Self {
        MemoryType::Uncacheable
    }
}

impl MemoryType {
    pub const fn into_bits(self) -> u64 {
        self as _
    }

    pub const fn from_bits(value: u64) -> Self {
        match value {
            0 => Self::Uncacheable,
            1 => Self::WriteCombinding,
            4 => Self::WriteThrough,
            5 => Self::WriteProtect,
            6 => Self::WriteBack,
            _ => Self::Uncacheable,
        }
    }
}

#[bitfield(u64)]
pub struct EPTP {
    #[bits(3)]
    pub memory_type: MemoryType,

    #[bits(3)]
    pub page_walk_length: u8,

    #[bits(1)]
    pub dirty_and_access: bool,

    #[bits(5)]
    _reserved_1: u8,

    #[bits(36)]
    pub pml4_address: u64,

    #[bits(16)]
    _reserved_2: u16,
}

/// PML4 entry which contains a 512-gb region
#[bitfield(u64)]
pub struct PML4E {
    #[bits(1)]
    pub read: bool,

    #[bits(1)]
    pub write: bool,

    #[bits(1)]
    pub execute: bool,

    #[bits(5)]
    _reserved_1: u8,

    #[bits(1)]
    pub accessed: bool,

    #[bits(1)]
    ignored_1: bool,

    #[bits(1)]
    pub execute_for_user_mode: bool,

    #[bits(1)]
    ignored_2: bool,

    #[bits(36)]
    pub page_frame_number: u64,

    #[bits(4)]
    _reserved_2: u16,

    #[bits(12)]
    _ignored_3: u16,
}

/// Page directory pointer table entry which contains a 1-gb region
#[bitfield(u64)]
pub struct PDPTE {
    #[bits(1)]
    pub read: bool,

    #[bits(1)]
    pub write: bool,

    #[bits(1)]
    pub execute: bool,

    #[bits(5)]
    _reserved_1: u8,

    #[bits(1)]
    pub accessed: bool,

    #[bits(1)]
    ignored_1: bool,

    #[bits(1)]
    pub execute_for_user_mode: bool,

    #[bits(1)]
    ignored_2: bool,

    #[bits(36)]
    pub page_frame_number: u64,

    #[bits(4)]
    _reserved_2: u16,

    #[bits(12)]
    _ignored_3: u16,
}

#[allow(non_camel_case_types)]
#[bitfield(u64)]
pub struct PDE_2MB {
    /// [Bit 0] Read access; indicates whether reads are allowed from the 2-MByte page referenced by this entry.
    #[bits(1)]
    pub read: bool,

    /// [Bit 1] Write access; indicates whether writes are allowed from the 2-MByte page referenced by this entry.
    #[bits(1)]
    pub write: bool,

    /// [Bit 2] If the "mode-based execute control for EPT" VM-execution control is 0, execute access; indicates whether
    /// instruction fetches are allowed from the 2-MByte page controlled by this entry.
    /// If that control is 1, execute access for supervisor-mode linear addresses; indicates whether instruction fetches are
    /// allowed from supervisor-mode linear addresses in the 2-MByte page controlled by this entry.
    #[bits(1)]
    pub execute: bool,

    /// [Bits 5:3] EPT memory type for this 2-MByte page.
    ///
    /// @see Vol3C[28.2.6(EPT and memory Typing)]
    #[bits(3)]
    pub memory_type: MemoryType,

    /// [Bit 6] Ignore PAT memory type for this 2-MByte page.
    ///
    /// @see Vol3C[28.2.6(EPT and memory Typing)]
    #[bits(1)]
    ignore_pat1: u64,

    /// [Bit 7] Must be 1 (otherwise, this entry references an EPT page table).
    #[bits(1)]
    pub large_page: bool,

    /// [Bit 8] If bit 6 of EPTP is 1, accessed flag for EPT; indicates whether software has accessed the 2-MByte page
    /// referenced by this entry. Ignored if bit 6 of EPTP is 0.
    ///
    /// @see Vol3C[28.2.4(Accessed and Dirty Flags for EPT)]
    #[bits(1)]
    pub accessed: bool,

    /// [Bit 9] If bit 6 of EPTP is 1, dirty flag for EPT; indicates whether software has written to the 2-MByte page referenced
    /// by this entry. Ignored if bit 6 of EPTP is 0.
    ///
    /// @see Vol3C[28.2.4(Accessed and Dirty Flags for EPT)]
    #[bits(1)]
    pub dirty: bool,

    /// [Bit 10] Execute access for user-mode linear addresses. If the "mode-based execute control for EPT" VM-execution control
    /// is 1, indicates whether instruction fetches are allowed from user-mode linear addresses in the 2-MByte page controlled
    /// by this entry. If that control is 0, this bit is ignored.
    #[bits(1)]
    pub user_mode_execute: bool,

    #[bits(10)]
    _reserved_1: u64,

    /// [Bits 47:21] Physical address of 4-KByte aligned EPT page-directory-pointer table referenced by this entry.
    #[bits(27)]
    pub page_frame_number: u64,

    #[bits(15)]
    _reserved_2: u64,

    /// [Bit 63] Suppress \#VE. If the "EPT-violation \#VE" VM-execution control is 1, EPT violations caused by accesses to this
    /// page are convertible to virtualization exceptions only if this bit is 0. If "EPT-violation \#VE" VMexecution control is
    /// 0, this bit is ignored.
    ///
    /// @see Vol3C[25.5.6.1(Convertible EPT Violations)]
    #[bits(1)]
    pub suppress_ve: bool,
}

/// Page directory entry which contains a 2-mb region
#[bitfield(u64)]
pub struct PDE {
    #[bits(1)]
    pub read: bool,

    #[bits(1)]
    pub write: bool,

    #[bits(1)]
    pub execute: bool,

    #[bits(5)]
    _reserved_1: u8,

    #[bits(1)]
    pub accessed: bool,

    #[bits(1)]
    ignored_1: bool,

    #[bits(1)]
    pub execute_for_user_mode: bool,

    #[bits(1)]
    ignored_2: bool,

    #[bits(36)]
    pub page_frame_number: usize,

    #[bits(4)]
    _reserved_2: u16,

    #[bits(12)]
    _ignored_3: u16,
}

/// Page table entry which maps a 4-kb page
#[bitfield(u64)]
pub struct PTE {
    #[bits(1)]
    pub read: bool,

    #[bits(1)]
    pub write: bool,

    #[bits(1)]
    pub execute: bool,

    #[bits(5)]
    _reserved_1: u8,

    #[bits(1)]
    pub accessed: bool,

    #[bits(1)]
    ignored_1: bool,

    #[bits(1)]
    pub execute_for_user_mode: bool,

    #[bits(1)]
    ignored_2: bool,

    #[bits(36)]
    pub physical_address: u64,

    #[bits(4)]
    _reserved_2: u16,

    #[bits(12)]
    _ignored_3: u16,
}

type PhysicalAddress = u64;
type VirtualAddress = u64;

#[derive(Default, Clone, Debug, Copy)]
pub struct MtrrRangeDescriptor {
    pub base_address: PhysicalAddress,
    pub length: PhysicalAddress,
    pub memory_type: MemoryType,
}

const MAX_MTRR_DESCRIPTORS: usize = 9;

#[derive(Default, Clone, Debug, Copy)]
pub struct MtrrMap {
    pub capability: Ia32MtrrCpabilitiesRegister,
    pub descriptors: [MtrrRangeDescriptor; MAX_MTRR_DESCRIPTORS],
    pub descriptor_count: usize,
}

pub fn read_mtrr() -> Result<MtrrMap> {
    let mut result = MtrrMap {
        capability: Ia32MtrrCpabilitiesRegister::new(),
        descriptors: [Default::default(); MAX_MTRR_DESCRIPTORS],
        descriptor_count: 0,
    };

    result.capability = unsafe { Ia32MtrrCpabilitiesRegister::from(rdmsr(IA32_MTRRCAP)) };
    for index in 0..result.capability.variable_range_count() {
        let phys_base =
            unsafe { Ia32MtrrPhysMaskRegister::from(rdmsr(IA32_MTRR_PHYSBASE0 + index as u32)) };

        let phys_mask =
            unsafe { Ia32MtrrPhysMaskRegister::from(rdmsr(IA32_MTRR_PHYSMASK0 + index as u32)) };

        if !phys_mask.valid() {
            continue;
        }

        if result.descriptor_count >= result.descriptors.len() {
            anyhow::bail!("too many variable MTRR capabilities")
        }

        let descriptor = &mut result.descriptors[result.descriptor_count];
        result.descriptor_count += 1;

        descriptor.base_address = phys_base.page_frame_number() * PAGE_SIZE;
        descriptor.length = 1 << (phys_mask.page_frame_number() * PAGE_SIZE).trailing_zeros();
        descriptor.memory_type = phys_base.memory_type();
    }

    Ok(result)
}

const TABLE_ENTRIES: usize = 512;
pub const PAGE_SIZE: u64 = 4096;

#[repr(align(4096))]
pub struct PagingTable<T: Copy> {
    pub entries: [T; TABLE_ENTRIES],
}

static EPT_ALLOCATOR: ContiguousMemoryAllocator = ContiguousMemoryAllocator::new(None);

#[repr(align(4096))]
pub struct VmPagingTable {
    pub pml4e: PagingTable<PML4E>,
    pub pml3e: PagingTable<PDPTE>,
    pub pml2e: [PagingTable<PDE_2MB>; TABLE_ENTRIES],
}

impl VmPagingTable {
    pub fn new_identity(mtrr: &MtrrMap) -> Box<Self, ContiguousMemoryAllocator> {
        let mut result =
            unsafe { Box::<VmPagingTable, _>::new_zeroed_in(EPT_ALLOCATOR).assume_init() };

        result.pml4e.entries[0] = PML4E::new()
            .with_page_frame_number(
                result.pml3e.entries.as_ptr().get_physical_address() as u64 / PAGE_SIZE,
            )
            .with_read(true)
            .with_write(true)
            .with_execute(true);

        for (index, entry) in result.pml3e.entries.iter_mut().enumerate() {
            *entry = PDPTE::new()
                .with_page_frame_number(
                    result.pml2e[index].entries.as_ptr().get_physical_address() as u64 / PAGE_SIZE,
                )
                .with_read(true)
                .with_write(true)
                .with_execute(true);
        }

        for pdpte_index in 0..TABLE_ENTRIES {
            for (pde_index, entry) in result.pml2e[pdpte_index].entries.iter_mut().enumerate() {
                let page_frame_number = (pdpte_index * TABLE_ENTRIES) + pde_index;

                let memory_type = if page_frame_number == 0 &&
                    mtrr.capability.fixed_range_supported()
                {
                    /*
                     * To be safe, we will map the first page as UC as to not bring up any kind of undefined behavior from the
                     * fixed MTRR section which we are not formally recognizing (typically there is MMIO memory in the first MB).
                     */
                    MemoryType::Uncacheable
                } else {
                    let page_start_address = (page_frame_number as u64) * 1024 * 1024 * 2;
                    let mut memory_type = MemoryType::WriteBack;
                    for descriptor in &mtrr.descriptors[0..mtrr.descriptor_count] {
                        if page_start_address > descriptor.base_address + descriptor.length {
                            continue;
                        }

                        if page_start_address + 1024 * 1024 * 2 <= descriptor.base_address {
                            continue;
                        }

                        memory_type = descriptor.memory_type;
                        if memory_type == MemoryType::Uncacheable {
                            /* No need to search for any other memory types as the page must be uncacheable. */
                            break;
                        }
                    }
                    memory_type
                };

                *entry = PDE_2MB::new()
                    .with_page_frame_number(page_frame_number as u64)
                    .with_memory_type(memory_type)
                    .with_large_page(true)
                    .with_read(true)
                    .with_write(true)
                    .with_execute(true);
            }
        }

        result
    }
}

#[cfg(test)]
mod test {
    use crate::ept::{
        MemoryType,
        EPTP,
        PDE,
        PDE_2MB,
        PDPTE,
        PML4E,
    };

    #[test]
    pub fn test_eptp() {
        assert_eq!(EPTP::new().with_memory_type(MemoryType::Uncacheable).0, 0);
        assert_eq!(EPTP::new().with_memory_type(MemoryType::WriteBack).0, 6);
        assert_eq!(
            EPTP::new()
                .with_memory_type(MemoryType::WriteBack)
                .with_pml4_address(0xDEADBEEF)
                .0,
            6 | (0xDEADBEEF << 12)
        );
    }

    #[test]
    pub fn test_pml4e() {
        assert_eq!(
            PML4E::new()
                .with_read(true)
                .with_execute(true)
                .with_write(true)
                .with_page_frame_number(0xDEADBEEF)
                .0,
            0x7 | (0xDEADBEEF << 12)
        );
    }

    #[test]
    pub fn test_pdpte() {
        assert_eq!(
            PDPTE::new()
                .with_read(true)
                .with_execute(true)
                .with_write(true)
                .with_page_frame_number(0xDEADBEEF)
                .0,
            0x7 | (0xDEADBEEF << 12)
        );
    }

    #[test]
    pub fn test_pde() {
        assert_eq!(
            PDE::new()
                .with_read(true)
                .with_execute(true)
                .with_write(true)
                .with_page_frame_number(0xDEADBEEF)
                .0,
            0x7 | (0xDEADBEEF << 12)
        );

        assert_eq!(
            PDE_2MB::new()
                .with_read(true)
                .with_execute(true)
                .with_write(true)
                .with_page_frame_number(0xDEAD)
                .0,
            0x7 | (0xDEAD << 21)
        );
    }
}
