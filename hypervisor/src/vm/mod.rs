use alloc::{
    boxed::Box,
    format,
    string::ToString,
};
use core::{
    arch::asm,
    sync::atomic::{
        AtomicU64,
        Ordering,
    },
};

use anyhow::Context;
use kapi::{
    KeGetCurrentIrql,
    DISPATCH_LEVEL,
};
use kapi_kmodule::{
    ByteSequencePattern,
    KModule,
    SearchPattern,
};
use obfstr::obfstr;
use x86::{
    controlregs,
    dtables::{
        self,
        DescriptorTablePointer,
    },
    msr::{
        self,
        IA32_FS_BASE,
        IA32_GS_BASE,
        IA32_SYSENTER_CS,
        IA32_SYSENTER_EIP,
        IA32_SYSENTER_ESP,
    },
    segmentation::{
        self,
        Descriptor,
    },
    vmx::vmcs::{
        self,
        control::{
            EntryControls,
            ExitControls,
            PinbasedControls,
            PrimaryControls,
            SecondaryControls,
        },
        ro::VM_INSTRUCTION_ERROR,
    },
};

use crate::{
    cpu_state,
    ept::{
        read_mtrr,
        MemoryType,
        VmPagingTable,
        EPTP,
        PAGE_SIZE,
    },
    mem::{
        MemoryAddress,
        MemoryAddressEx,
    },
    processor,
    utils,
    vmx::{
        self,
        vm_write,
        VmGuestConfiguration,
        VmxControl,
    },
};

mod exit;
mod exit_handler;

static SEH_VMCALL_TARGET: AtomicU64 = AtomicU64::new(0);
fn resolve_vmcall_target() -> anyhow::Result<u64> {
    let kernel_base = KModule::find_by_name(obfstr!("ntoskrnl.exe"))?
        .with_context(|| obfstr!("could not find kernel base").to_string())?;

    let pattern = ByteSequencePattern::parse(obfstr!("0F 01 C1 C3")).with_context(|| {
        obfstr!("could not compile HvipApertureIntelVmcall pattern").to_string()
    })?;

    let vmcall_target = kernel_base
        .find_code_sections()?
        .into_iter()
        .find_map(|section| {
            if let Some(data) = section.raw_data() {
                if let Some(offset) = pattern.find(data) {
                    Some(offset + section.raw_data_address())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .with_context(|| {
            format!(
                "failed to find {} pattern",
                obfstr!("HvipApertureIntelVmcall")
            )
        })? as u64;

    log::trace!(
        "{} {:X} ({:X})",
        obfstr!("SEH found HvipApertureIntelVmcall at"),
        vmcall_target - kernel_base.base_address as u64,
        vmcall_target
    );

    Ok(vmcall_target)
}

pub fn virtualize_current_system() -> anyhow::Result<()> {
    {
        let seh_vmcall = resolve_vmcall_target()?;
        SEH_VMCALL_TARGET.store(seh_vmcall, Ordering::Relaxed);
    }

    let host_cr3 = unsafe { controlregs::cr3() };
    for state in cpu_state::all() {
        state.host_cr3 = host_cr3;
    }

    /* Enable the VMX CPU feature via MSR */
    vmx::feature_enable()?;

    let mtrr = read_mtrr()?;
    let ept = VmPagingTable::new_identity(&mtrr);
    log::debug!(
        "PML4E = {:X}",
        ept.pml4e.entries.as_ptr().get_physical_address() as u64
    );

    for state in cpu_state::all() {
        state.eptp = EPTP::new()
            .with_memory_type(MemoryType::WriteBack)
            .with_page_walk_length(3)
            .with_pml4_address(ept.pml4e.entries.as_ptr().get_physical_address() as u64 / PAGE_SIZE)
            .with_dirty_and_access(true);
    }

    // FIXME: Memory leak!
    Box::leak(ept);

    processor::run_on_all(virtualize_current_system_current_cpu);
    Ok(())
}

fn virtualize_current_system_current_cpu() {
    assert!(KeGetCurrentIrql() >= DISPATCH_LEVEL);
    let state = cpu_state::current();

    if !state.vmxon_active {
        if let Err(err) = vmx::enable_current_cpu() {
            log::error!(
                "{} {}: {:?}",
                state.processor_index,
                obfstr!("Failed to enable VMX"),
                err
            );
            return;
        }
    }

    {
        let vmcs_address = state.vmcs.get_physical_address() as u64;
        // "the VMCLEAR instruction initializes any implementation-specific
        //  information in the VMCS region referenced by its operand. (...),
        //  software should execute VMCLEAR on a VMCS region before making the
        //  corresponding VMCS active with VMPTRLD for the first time."
        // See: 25.11.3 Initializing a VMCS
        let result = unsafe { vmx::vmclear(vmcs_address) };
        if let Err(err) = result {
            log::error!(
                "{} {}: {:?}",
                state.processor_index,
                obfstr!("Failed to clear current VMCS block: {:?}"),
                err
            );
            return;
        }

        let result = unsafe { vmx::vmptrld(vmcs_address) };
        if let Err(err) = result {
            log::error!(
                "{} {}: {:?}",
                state.processor_index,
                obfstr!("Failed to setup VMCS"),
                err
            );
            return;
        }
    }
    state.vmcs_active = true;
    log::debug!("{} {}", state.processor_index, obfstr!("VMCS enabled"));

    let vmcs_error = Ok(())
        .and_then(|_| vmcs_setup_host().with_context(|| obfstr!("vmcs host").to_string()))
        .and_then(|_| vmcs_setup_guest().with_context(|| obfstr!("vmcs guest").to_string()))
        .and_then(|_| vmcs_setup_controls().with_context(|| obfstr!("vmcs controls").to_string()));

    if let Err(err) = vmcs_error {
        log::error!(
            "{} {}: {:?}",
            state.processor_index,
            obfstr!("VMCS setup failed"),
            err
        );
        return;
    }

    log::debug!("{} Executing VMLAUNCH", state.processor_index);
    let start_result: u64;
    unsafe {
        asm!(
            /*
             * Set RSP to the current stack ptr.
             * Set RIP to the success label to indicate launch success.
             */
            "lea {tmp}, [rip + 1f]",
            "vmwrite {vmcs_guest_rsp:r}, rsp",
            "vmwrite {vmcs_guest_rip:r}, {tmp}",

            /* Try to launch the VM. The entry point should be label 1. */
            "vmlaunch",

            /* Handle vmlaunch failure as we didn't jumped to label 1. */
            "mov {res}, 0",
            "jmp 2f",

            /* vm launched successfull */
            "1:",
            "mov {res}, 1",

            "2:",
            tmp = out(reg) _,
            res = out(reg) start_result,
            vmcs_guest_rsp = in(reg) vmcs::guest::RSP as u64,
            vmcs_guest_rip = in(reg) vmcs::guest::RIP as u64,
        );
    }

    /* Invalidate memory contexts */
    //mem::invvpid(mem::InvVpidMode::AllContext);
    // unsafe {
    //     //instructions::tlb::flush_pcid(instructions::tlb::InvPicdCommand::All);
    // }

    if start_result > 0 {
        state.vm_launched = true;

        /* TODO: Test vmcall handler */
        log::info!("{} Core virtualised!", state.processor_index);
        return;
    }

    /* if we return from VM launch something went wrong */
    let error_code = unsafe { vmx::vmread(VM_INSTRUCTION_ERROR) };
    log::error!(
        "{} Failed to launch VM: {:?}",
        state.processor_index,
        error_code
    );
}

pub fn exit_virtualisation() {
    processor::run_on_all(exit_virtualisation_current_cpu);
}

fn exit_virtualisation_current_cpu() {
    assert!(KeGetCurrentIrql() >= DISPATCH_LEVEL);

    /*
     * As cpu_states are hidden when using our own hypervisor we do
     * not know if we're using any hypervisor. Therefor just try and
     * detect failure.
     */

    let target_fn = SEH_VMCALL_TARGET.load(Ordering::Relaxed);
    if target_fn == 0 {
        log::warn!(
            "{} {}",
            processor::current(),
            obfstr!("Failed to resolve VM imports")
        );
        return;
    }

    let result = unsafe { seh::wrapper::seh_invoke(target_fn, 0x0DEADBEEF, 0x056485456, 0, 0) };
    if !result {
        log::debug!(
            "{} Processor not virtualized via vthv",
            processor::current()
        );
        return;
    }

    log::debug!("{} Virtualization exited", processor::current());
}

fn vmcs_setup_host() -> anyhow::Result<()> {
    let state = cpu_state::current();

    let mut gdt = DescriptorTablePointer::<Descriptor>::default();
    let mut idt = DescriptorTablePointer::<Descriptor>::default();

    unsafe {
        dtables::sgdt(&mut gdt);
        dtables::sidt(&mut idt);

        const RPL_MASK: u16 = 0xF8;
        vm_write!(
            vmcs::host::CS_SELECTOR,
            segmentation::cs().bits() & RPL_MASK
        )?;
        vm_write!(
            vmcs::host::SS_SELECTOR,
            segmentation::ss().bits() & RPL_MASK
        )?;
        vm_write!(
            vmcs::host::DS_SELECTOR,
            segmentation::ds().bits() & RPL_MASK
        )?;
        vm_write!(
            vmcs::host::ES_SELECTOR,
            segmentation::es().bits() & RPL_MASK
        )?;
        vm_write!(
            vmcs::host::FS_SELECTOR,
            segmentation::fs().bits() & RPL_MASK
        )?;
        vm_write!(vmcs::host::FS_BASE, msr::rdmsr(IA32_FS_BASE))?;
        vm_write!(
            vmcs::host::GS_SELECTOR,
            segmentation::gs().bits() & RPL_MASK
        )?;
        vm_write!(vmcs::host::GS_BASE, msr::rdmsr(IA32_GS_BASE))?;

        let tr = x86::task::tr();
        vm_write!(vmcs::host::TR_SELECTOR, tr.bits() & RPL_MASK)?;
        vm_write!(
            vmcs::host::TR_BASE,
            utils::get_segment_base(&gdt, tr) as u64
        )?;

        vm_write!(vmcs::host::GDTR_BASE, gdt.base as u64)?;
        vm_write!(vmcs::host::IDTR_BASE, idt.base as u64)?;

        /* FIXME: Do not share the same IDT due to nmi! Create an own host IDT :) */
        // https://www.unknowncheats.me/forum/c-and-c-/390593-vm-escape-via-nmi.html

        vm_write!(vmcs::host::IA32_SYSENTER_CS, msr::rdmsr(IA32_SYSENTER_CS))?;
        vm_write!(vmcs::host::IA32_SYSENTER_EIP, msr::rdmsr(IA32_SYSENTER_EIP))?;
        vm_write!(vmcs::host::IA32_SYSENTER_ESP, msr::rdmsr(IA32_SYSENTER_ESP))?;

        /*
         * We may be executing in an arbitrary user-mode, process as part of the DPC interrupt we execute in.
         * We may need to use a cr3 which has been previously saved.
         */
        vm_write!(vmcs::host::CR3, state.host_cr3)?;
        vm_write!(vmcs::host::CR0, controlregs::cr0().bits() as u64)?;
        vm_write!(vmcs::host::CR4, controlregs::cr4().bits() as u64)?;

        vm_write!(
            vmcs::host::RSP,
            state.vmm_stack.as_ptr().byte_add(state.vmm_stack.len() - 1) as u64
        )?;
        vm_write!(vmcs::host::RIP, exit::vmexit_handler as u64)?;
    }

    Ok(())
}

fn vmcs_setup_guest() -> anyhow::Result<()> {
    /* entry rsp & rip will be set when known */
    let guest_config = VmGuestConfiguration::from_current_host(0, 0);

    guest_config.apply()
}

unsafe fn apply_control(control: impl Into<VmxControl>) -> anyhow::Result<()> {
    let control: VmxControl = control.into();
    log::debug!(
        "Set {:?} to {:X} (requested: {:X})",
        control,
        control.adjusted_value(),
        control.value()
    );
    vm_write!(control.vmcs_field(), control.adjusted_value())?;
    Ok(())
}

fn vmcs_setup_controls() -> anyhow::Result<()> {
    let state = cpu_state::current();

    unsafe {
        vm_write!(vmcs::control::TSC_OFFSET_FULL, 0)?;
        vm_write!(vmcs::control::TSC_OFFSET_HIGH, 0)?;

        vm_write!(vmcs::control::PAGE_FAULT_ERR_CODE_MASK, 0)?;
        vm_write!(vmcs::control::PAGE_FAULT_ERR_CODE_MATCH, 0)?;

        vm_write!(vmcs::control::VMEXIT_MSR_STORE_COUNT, 0)?;
        vm_write!(vmcs::control::VMEXIT_MSR_LOAD_COUNT, 0)?;

        vm_write!(vmcs::control::VMENTRY_MSR_LOAD_COUNT, 0)?;
        vm_write!(vmcs::control::VMENTRY_INTERRUPTION_INFO_FIELD, 0)?;

        /* TODO: Is this correct or shall we all assign the same VPID? */
        vm_write!(vmcs::control::VPID, 1)?;

        apply_control(ExitControls::HOST_ADDRESS_SPACE_SIZE | ExitControls::ACK_INTERRUPT_ON_EXIT)?;
        apply_control(EntryControls::IA32E_MODE_GUEST)?;
        apply_control(PinbasedControls::empty())?;
        apply_control(
            PrimaryControls::SECONDARY_CONTROLS | PrimaryControls::CR3_LOAD_EXITING, // | PrimaryControls::INVLPG_EXITING,
        )?; // | IA32_VMX_PROCBASED_CTLS_ACTIVATE_MSR_BITMAP,
        apply_control(
            SecondaryControls::ENABLE_VPID |
                SecondaryControls::ENABLE_INVPCID |
                SecondaryControls::ENABLE_RDTSCP |
                SecondaryControls::ENABLE_XSAVES_XRSTORS |
                SecondaryControls::ENABLE_EPT,
        )?;

        vm_write!(vmcs::control::CR0_GUEST_HOST_MASK, 0)?;
        vm_write!(vmcs::control::CR0_READ_SHADOW, 0)?;

        vm_write!(vmcs::control::CR3_TARGET_COUNT, 0)?;
        vm_write!(vmcs::control::CR3_TARGET_VALUE0, 0)?;
        vm_write!(vmcs::control::CR3_TARGET_VALUE1, 0)?;
        vm_write!(vmcs::control::CR3_TARGET_VALUE2, 0)?;
        vm_write!(vmcs::control::CR3_TARGET_VALUE3, 0)?;

        vm_write!(vmcs::control::CR4_GUEST_HOST_MASK, 0)?;
        vm_write!(vmcs::control::CR4_READ_SHADOW, 0)?;

        let msr_bitmap =
            MemoryAddress::Virtual(&*state.msr_bitmap as *const _ as usize).physical_address();
        vm_write!(vmcs::control::MSR_BITMAPS_ADDR_FULL, msr_bitmap as u64)?;

        let eptp: u64 = state.eptp.into();
        vm_write!(vmcs::control::EPTP_FULL, eptp)?;
        // // Set up EPT
        // __vmx_vmwrite(EPT_POINTER, EptState->EptPointer.Flags);
    }

    Ok(())
}
