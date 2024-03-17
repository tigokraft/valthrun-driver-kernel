use alloc::boxed::Box;

use anyhow::Result;
use bitfield_struct::bitfield;
use kalloc::ContiguousMemoryAllocator;
use x86::{
    bits64::paging::{
        self,
        PDEntry,
        PDFlags,
        PDPTEntry,
        PDPTFlags,
        PML4Flags,
        LARGE_PAGE_SIZE,
        PAGE_SIZE_ENTRIES,
    },
    msr::{
        rdmsr,
        IA32_MTRRCAP,
        IA32_MTRR_PHYSBASE0,
        IA32_MTRR_PHYSMASK0,
    },
};

use crate::{
    mem::MemoryAddressEx,
    msr::{
        Ia32MtrrCpabilitiesRegister,
        Ia32MtrrPhysMaskRegister,
    },
};

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

type PhysicalAddress = u64;

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

pub const PAGE_SIZE: u64 = 4096;

static EPT_ALLOCATOR: ContiguousMemoryAllocator = ContiguousMemoryAllocator::new(None);

#[repr(C, align(4096))]
pub struct VmPagingTable {
    pub pml4e: paging::PML4,
    pub pml3e: paging::PDPT,
    pub pml2e: [paging::PD; PAGE_SIZE_ENTRIES],
}

impl VmPagingTable {
    pub fn new_identity(mtrr: &MtrrMap) -> Box<Self, ContiguousMemoryAllocator> {
        let mut result =
            unsafe { Box::<VmPagingTable, _>::new_zeroed_in(EPT_ALLOCATOR).assume_init() };

        result.pml4e[0] = paging::PML4Entry::new(
            result.pml3e.as_ptr().get_physical_address(),
            PML4Flags::P | PML4Flags::RW | PML4Flags::US,
        );

        for (index, entry) in result.pml3e.iter_mut().enumerate() {
            *entry = PDPTEntry::new(
                result.pml2e[index].as_ptr().get_physical_address(),
                PDPTFlags::P | PDPTFlags::RW | PDPTFlags::US,
            );
        }

        for pdpte_index in 0..PAGE_SIZE_ENTRIES {
            for (pde_index, entry) in result.pml2e[pdpte_index].iter_mut().enumerate() {
                let page_frame_number = (pdpte_index * PAGE_SIZE_ENTRIES) + pde_index;

                let memory_type = if page_frame_number == 0 &&
                    mtrr.capability.fixed_range_supported()
                {
                    /*
                     * To be safe, we will map the first page as UC as to not bring up any kind of undefined behavior from the
                     * fixed MTRR section which we are not formally recognizing (typically there is MMIO memory in the first MB).
                     */
                    MemoryType::Uncacheable
                } else {
                    let page_start_address = (page_frame_number * LARGE_PAGE_SIZE) as u64;
                    let mut memory_type = MemoryType::WriteBack;
                    for descriptor in &mtrr.descriptors[0..mtrr.descriptor_count] {
                        if page_start_address > descriptor.base_address + descriptor.length {
                            continue;
                        }

                        if page_start_address + LARGE_PAGE_SIZE as u64 <= descriptor.base_address {
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

                let memory_type_flags = if matches!(memory_type, MemoryType::WriteBack) {
                    //PDFlags::PWT
                    PDFlags::empty()
                } else {
                    PDFlags::empty()
                };

                *entry = PDEntry::new(
                    (page_frame_number * LARGE_PAGE_SIZE).into(),
                    PDFlags::P | PDFlags::RW | PDFlags::US | PDFlags::PS | memory_type_flags,
                );
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
}
