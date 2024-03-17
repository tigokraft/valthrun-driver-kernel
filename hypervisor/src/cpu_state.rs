use alloc::{
    boxed::Box,
    vec::Vec,
};
use core::cell::SyncUnsafeCell;

use obfstr::obfstr;
use x86::msr::{
    self,
    IA32_VMX_BASIC,
};

use crate::{
    ept::EPTP,
    mem::MemoryAddressEx,
    msr::Ia32VmxBasicMsr,
    processor,
    vmx::{
        MsrBitmap,
        Vmcs,
        Vmxon,
    },
};

const VMM_STACK_SIZE: usize = 4096 * 2;
const ALIGNMENT_PAGE_SIZE: usize = 4096;

pub struct CpuState {
    pub processor_index: usize,

    pub vmxon: Box<Vmxon>,
    pub vmxon_active: bool,

    pub vmcs: Box<Vmcs>,
    pub vmcs_active: bool,

    pub vm_launched: bool,
    pub vm_exit_scheduled: bool,

    pub vmx_root_mode: bool,
    pub vmx_root_incr_rip: bool,

    pub vmm_stack: Box<[u8]>,
    pub msr_bitmap: Box<MsrBitmap>,

    pub host_cr3: u64,
    pub eptp: EPTP,
}

static GUEST_STATE: SyncUnsafeCell<Option<Box<[CpuState]>>> = SyncUnsafeCell::new(None);

pub fn allocate() -> anyhow::Result<()> {
    let states = unsafe { &mut *GUEST_STATE.get() };
    assert!(states.is_none());

    let vmx_basic = unsafe { Ia32VmxBasicMsr::from(msr::rdmsr(IA32_VMX_BASIC)) };
    log::debug!(
        "{} {}",
        obfstr!("MSR_IA32_VMX_BASIC (MSR 0x480) revision is"),
        vmx_basic.revision_identifier()
    );

    let mut state_memory = Vec::new();
    for index in 0..processor::active_count() {
        /* Allocating slices of 4096 bytes should always be aligned to 4096 bytes */
        let vmxon = {
            let mut vmxon = unsafe { Box::<Vmxon>::new_zeroed().assume_init() };
            vmxon.revision_id = vmx_basic.revision_identifier();

            let vmxon_phys = vmxon.get_physical_address();
            if !vmxon_phys.is_base_page_aligned() {
                anyhow::bail!("failed to allocate aligned vmxon memory");
            }
            log::trace!(
                "{} VMXON at {:X} (virt: {:X})",
                index,
                vmxon_phys.0,
                &*vmxon as *const _ as u64
            );

            vmxon
        };

        let vmcs = {
            let mut vmcs = unsafe { Box::<Vmcs>::new_zeroed().assume_init() };
            vmcs.revision_id = vmx_basic.revision_identifier();

            let vmcs_phys = vmcs.get_physical_address();
            if !vmcs_phys.is_base_page_aligned() {
                anyhow::bail!("failed to allocate aligned vmcs memory");
            }
            log::trace!(
                "{} VMCS at {:X} (virt: {:X})",
                index,
                vmcs_phys.0,
                &*vmcs as *const _ as u64
            );

            vmcs
        };

        let vmm_stack = {
            let mut memory = Vec::new();
            memory.resize(VMM_STACK_SIZE, 0u8);

            memory.into_boxed_slice()
        };

        let msr_bitmap = unsafe { Box::<MsrBitmap>::new_zeroed().assume_init() };

        state_memory.push(CpuState {
            processor_index: index,

            vmxon,
            vmxon_active: false,

            vmcs,
            vmcs_active: false,

            msr_bitmap,
            vmm_stack,

            vm_exit_scheduled: false,
            vm_launched: false,

            vmx_root_incr_rip: false,
            vmx_root_mode: false,

            host_cr3: 0,
            eptp: EPTP::new(),
        });
    }
    *states = Some(state_memory.into_boxed_slice());

    Ok(())
}

pub fn free() {
    let states = unsafe { &mut *GUEST_STATE.get() };
    if let Some(memory) = states.take() {
        drop(memory);
    }
}

pub fn all() -> &'static mut [CpuState] {
    let states = unsafe { &mut *GUEST_STATE.get() };
    states.as_mut().unwrap()
}

pub fn current() -> &'static mut CpuState {
    let states = all();
    let current_processor = processor::current();
    if current_processor >= states.len() {
        panic!(
            "CPU {} tried to access his local state but only {} states allocated.",
            current_processor,
            states.len()
        );
    }

    &mut states[current_processor]
}

pub fn try_current() -> Option<&'static mut CpuState> {
    let states = unsafe { &mut *GUEST_STATE.get() };
    let current_processor = processor::current();
    states
        .as_mut()
        .map(|states| states.get_mut(current_processor))
        .flatten()
}
