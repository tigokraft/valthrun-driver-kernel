use core::arch::asm;

use x86::{
    cpuid::cpuid,
    current::vmx,
    msr,
    vmx::vmcs,
};

use crate::{
    mem,
    vmx::{
        ControlRegister,
        CpuRegisters,
        MovCrQualification,
    },
};

pub fn handle_cpuid(guest_state: &mut CpuRegisters) {
    // log::debug!(
    //     "CPU ID {:X}, {:X} at {:X} (rsp: {:X}, cr3: {:X})",
    //     guest_state.rax,
    //     guest_state.rcx,
    //     guest_state.rip(),
    //     guest_state.rsp(),
    //     guest_state.cr3()
    // );
    let mut info = cpuid!(guest_state.rax, guest_state.rcx);
    let request = guest_state.rax & 0xFFFFFFFF;
    if request == 1 {
        /* Hypervisor present bit */
        // info.ecx |= 0x80000000;

        /* Disable MTRR */
        // info.edx &= !(1 << 12);

        /* Disable XSAVE */
        // info.ecx &= !(1 << 26);

        /* Hide TSC deadline timer */
        // info.ecx &= !(1 << 24);
    } else if request == 0x40000001 {
        info.eax = 0x56485456; /* VTHV */
    }

    guest_state.rax &= !0xFFFFFFFF;
    guest_state.rbx &= !0xFFFFFFFF;
    guest_state.rcx &= !0xFFFFFFFF;
    guest_state.rdx &= !0xFFFFFFFF;

    guest_state.rax |= info.eax as u64;
    guest_state.rbx |= info.ebx as u64;
    guest_state.rcx |= info.ecx as u64;
    guest_state.rdx |= info.edx as u64;
}

pub fn handle_cr_access(guest_state: &mut CpuRegisters) {
    let qualification =
        unsafe { MovCrQualification::from(vmx::vmread(vmcs::ro::EXIT_QUALIFICATION).unwrap()) };
    log::debug!("CR: {:?}", qualification);

    unsafe {
        let reg_ptr = (&mut guest_state.rax as *mut u64).offset(qualification.register() as isize);
        match qualification.access_type() {
            0 => {
                /* TYPE_MOV_TO_CR */
                let value = guest_state.read_gp_register(qualification.register());
                match qualification.control_register() {
                    ControlRegister::Cr0 => {
                        let _ = vmx::vmwrite(vmcs::guest::CR0, value);
                        let _ = vmx::vmwrite(vmcs::control::CR0_READ_SHADOW, value);
                    }
                    ControlRegister::Cr3 => {
                        log::debug!("Updated Cr3 from {:X} to {:X}", guest_state.cr3(), value);
                        let _ = vmx::vmwrite(vmcs::guest::CR3, value & !(1 << 63));
                        mem::invvpid(mem::InvVpidMode::AllContext); // Invalid tlb
                                                                    // x86_64::instructions::tlb::flush_pcid(
                                                                    //     x86_64::instructions::tlb::InvPicdCommand::All,
                                                                    // );
                        x86::tlb::flush_all();
                    }
                    ControlRegister::Cr4 => {
                        let _ = vmx::vmwrite(vmcs::guest::CR4, value);
                        let _ = vmx::vmwrite(vmcs::control::CR4_READ_SHADOW, value);
                    }
                }
            }
            1 => {
                /* TYPE_MOV_FROM_CR */
                let vmcs_field = match qualification.control_register() {
                    ControlRegister::Cr0 => vmcs::guest::CR0,
                    ControlRegister::Cr3 => vmcs::guest::CR3,
                    ControlRegister::Cr4 => vmcs::guest::CR4,
                };

                let value = vmx::vmread(vmcs_field).unwrap_or(0);
                guest_state.write_gp_register(qualification.register(), value);
            }
            _ => asm!("int 3"),
        }
    }
}

pub fn handle_msr_read(guest_state: &mut CpuRegisters) {
    // log::trace!("MSR read {:X}", guest_state.rcx);
    let value = unsafe { msr::rdmsr(guest_state.rcx as u32) };
    guest_state.rdx = value >> 32;
    guest_state.rax = value & 0xFFFFFFFF;
}

pub fn handle_msr_write(guest_state: &mut CpuRegisters) {
    let value = (guest_state.rdx << 32) | (guest_state.rax & 0xFFFFFFFF);
    // log::trace!("MSR write {:X} := {:X}", guest_state.rcx, value);
    unsafe { msr::wrmsr(guest_state.rcx as u32, value) }
}
