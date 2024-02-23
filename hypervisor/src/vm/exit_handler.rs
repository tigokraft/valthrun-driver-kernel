use core::arch::asm;

use bitfield_struct::bitfield;
use x86::{
    cpuid::cpuid,
    current::vmx,
    msr,
    vmx::vmcs,
};

use crate::{
    mem,
    vmx::CpuRegisters,
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

#[bitfield(u64)]
pub struct MovCrQualification {
    #[bits(4)]
    pub control_register: u8,

    #[bits(2)]
    pub access_type: u8,
    pub lmsw_operand_type: bool,

    _reserved1: bool,

    #[bits(4)]
    pub register: u8,

    #[bits(4)]
    _reserved2: u8,

    #[bits(16)]
    pub lmsw_source_data: u16,

    #[bits(32)]
    _reserved3b: u32,
}

pub fn handle_cr_access(guest_state: &mut CpuRegisters) {
    unsafe { asm!("int 3") };
    /* TODO: HvHandleControlRegisterAccess */
    let qualification =
        unsafe { MovCrQualification::from(vmx::vmread(vmcs::ro::EXIT_QUALIFICATION).unwrap()) };

    unsafe {
        let reg_ptr = (&mut guest_state.rax as *mut u64).offset(qualification.register() as isize);
        if qualification.register() == 4 {
            /* TODO! */
            asm!("int 3");
        }

        match qualification.access_type() {
            0 => {
                /* TYPE_MOV_TO_CR */
                match qualification.control_register() {
                    0 => {
                        let _ = vmx::vmwrite(vmcs::guest::CR0, *reg_ptr);
                        let _ = vmx::vmwrite(vmcs::control::CR0_READ_SHADOW, *reg_ptr);
                    }
                    3 => {
                        let _ = vmx::vmwrite(vmcs::guest::CR3, *reg_ptr);
                        mem::invvpid(mem::InvVpidMode::SingleContext(0x01));
                    }
                    4 => {
                        let _ = vmx::vmwrite(vmcs::guest::CR4, *reg_ptr);
                        let _ = vmx::vmwrite(vmcs::control::CR4_READ_SHADOW, *reg_ptr);
                    }
                    _ => asm!("int 3"),
                }
            }
            1 => {
                /* TYPE_MOV_FROM_CR */
                match qualification.control_register() {
                    0 => {
                        *reg_ptr = vmx::vmread(vmcs::guest::CR0).unwrap_or(0);
                    }
                    3 => {
                        *reg_ptr = vmx::vmread(vmcs::guest::CR3).unwrap_or(0);
                    }
                    4 => {
                        *reg_ptr = vmx::vmread(vmcs::guest::CR4).unwrap_or(0);
                    }
                    _ => asm!("int 3"),
                }
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
