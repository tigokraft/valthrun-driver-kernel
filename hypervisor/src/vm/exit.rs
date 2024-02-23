use core::arch::asm;

use x86::{
    controlregs,
    current::vmx,
    dtables::{
        self,
        DescriptorTablePointer,
    },
    msr::{
        self,
        IA32_FS_BASE,
        IA32_GS_BASE,
    },
    segmentation::Descriptor,
    vmx::vmcs::{
        self,
        ro::VM_INSTRUCTION_ERROR,
    },
};

use super::exit_handler;
use crate::{
    cpu_states,
    vmx::{
        CpuRegisters,
        ExitInformation,
        ExitReason,
    },
};

#[naked]
pub unsafe extern "system" fn vmexit_handler() {
    asm!(
        /* Align stack */
        "and rsp, -10h",

        /*
         * Save the XMM[0:5] registers as Rust uses them and they are volatile.
         * YMM[0:5] registers are volatile as well, but we do not use them.
         */
        "sub rsp, 60h",
        "movaps [rsp + 5 * 16], xmm0",
        "movaps [rsp + 4 * 16], xmm1",
        "movaps [rsp + 3 * 16], xmm2",
        "movaps [rsp + 2 * 16], xmm3",
        "movaps [rsp + 1 * 16], xmm4",
        "movaps [rsp + 0 * 16], xmm5",

        /* Dummy space which we can use to store the exit rsp on */
        "push 0",

        /* must be in reverse order of CpuRegisters */
        "push r15",
        "push r14",
        "push r13",
        "push r12",
        "push r11",
        "push r10",
        "push r9",
        "push r8",
        "push rdi",
        "push rsi",
        "push rbp",
        "push rbp", /* RBP as RSP placeholder */
        "push rbx",
        "push rdx",
        "push rcx",
        "push rax",
        "pushfq",

        /* First Arg for &mut CpuRegisters */
        "mov rcx, rsp",

        /* Call the Rust handler */
        "sub rsp, 28h",
        "call {callback_vmexit}",
        "add rsp, 28h",

        /* Call the vmoff callback when the handler requests exit */
        "cmp al, 1",
        "je 1f",

        /* must be in order of CpuRegisters */
        "popfq",
        "pop rax",
        "pop rcx",
        "pop rdx",
        "pop rbx",
        "pop rbp", /* the stack ptr */
        "pop rbp",
        "pop rsi",
        "pop rdi",
        "pop r8",
        "pop r9",
        "pop r10",
        "pop r11",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",

        /* the dummy value */
        "add rsp, 8h",

        "movaps xmm5, [rsp + 0 * 16]",
        "movaps xmm4, [rsp + 1 * 16]",
        "movaps xmm3, [rsp + 2 * 16]",
        "movaps xmm2, [rsp + 3 * 16]",
        "movaps xmm1, [rsp + 4 * 16]",
        "movaps xmm0, [rsp + 5 * 16]",
        "add rsp, 60h",

        "vmresume",
        "jmp {callback_vmresume_failed}",

        "1:",
        "mov rcx, rsp", /* ptr to the guest registers */
        "jmp {callback_vmoff_execute}",

        callback_vmexit = sym callback_vmexit,
        callback_vmresume_failed = sym callback_vmresume_failed,
        callback_vmoff_execute = sym callback_vmoff_execute,
        options(noreturn)
    )
}

fn prepare_vm_exit() {
    let state = cpu_states::current();
    if state.vm_exit_scheduled {
        return;
    }

    /* Copy new host state from virtualized host */
    unsafe {
        /* Update the current CR3 from the guest as we might be in some arbitrary user mode */
        controlregs::cr3_write(vmx::vmread(vmcs::guest::CR3).unwrap_or(0));

        msr::wrmsr(IA32_FS_BASE, vmx::vmread(vmcs::guest::FS_BASE).unwrap());
        msr::wrmsr(IA32_GS_BASE, vmx::vmread(vmcs::guest::GS_BASE).unwrap());

        let mut gdt = DescriptorTablePointer::<Descriptor>::default();
        gdt.base = vmx::vmread(vmcs::guest::GDTR_BASE).unwrap() as *const _;
        gdt.limit = vmx::vmread(vmcs::guest::GDTR_LIMIT).unwrap() as u16;
        dtables::lgdt(&gdt);

        let mut idt = DescriptorTablePointer::<Descriptor>::default();
        idt.base = vmx::vmread(vmcs::guest::IDTR_BASE).unwrap() as *const _;
        idt.limit = vmx::vmread(vmcs::guest::IDTR_LIMIT).unwrap() as u16;
        dtables::lidt(&idt);
    }

    state.vm_exit_scheduled = true;
}

extern "system" fn callback_vmexit(registers: &mut CpuRegisters) -> bool {
    let state = cpu_states::current();

    state.vmx_root_incr_rip = true;
    state.vmx_root_mode = true;

    let exit_info = unsafe { vmx::vmread(vmcs::ro::EXIT_REASON).ok() }
        .map(ExitInformation::from_vmexit_code)
        .flatten()
        .expect("the exit code to contain a valid exit reason");

    if !matches!(
        exit_info.reason,
        ExitReason::RdMsr | ExitReason::WrMsr | ExitReason::CpuId
    ) {
        log::trace!(
            "VM exit {:?} | Guest RIP: {:X} RSP: {:X}, RBP: {:X}",
            exit_info.reason,
            unsafe { vmx::vmread(vmcs::guest::RIP).unwrap_or(0) },
            unsafe { vmx::vmread(vmcs::guest::RSP).unwrap_or(0) },
            registers.rbp
        );
    }

    match exit_info.reason {
        ExitReason::TripleFault => {
            unsafe {
                asm!("int 3");
            }
            state.vmx_root_incr_rip = true;
        }
        ExitReason::IoInstruction => {
            unsafe {
                asm!("int 3");
            };
        }

        ExitReason::VmClear
        | ExitReason::VmPtrLd
        | ExitReason::VmPtrRst
        | ExitReason::VmRead
        | ExitReason::VmWrite
        | ExitReason::VmResume
        | ExitReason::VmxOff
        | ExitReason::VmxOn
        | ExitReason::VmLaunch => {
            log::debug!("VMX instruction {:?} from {:X}", exit_info.reason, unsafe {
                vmx::vmread(vmcs::guest::RIP).unwrap_or(0)
            });

            /*
             * Target guest tries to execute VM Instruction, it probably causes a fatal error or system halt as the system might
             * think it has VMX feature enabled while it's not available due to our use of hypervisor.
             */
            /* FIXME: Signal uo hw exception / https://revers.engineering/day-5-vmexits-interrupts-cpuid-emulation/ */
            unsafe {
                /* 0x01 indicate vm instructions fail */
                let _ = vmx::vmwrite(
                    vmcs::guest::RFLAGS,
                    vmx::vmread(vmcs::guest::RFLAGS).unwrap_or(0) | 0x01,
                );
            }
        }

        ExitReason::VmCall => {
            if registers.rdx == 0x56485456 && registers.rcx == 0xDEADBEEF {
                /* Shutdown VMX */
                log::debug!("VM shutdown received.");
                prepare_vm_exit();
            } else {
                log::debug!(
                    "VMCALL rax = {:X}, rcx = {:X}, rdx = {:X}, cr3 = {:X}",
                    registers.rax,
                    registers.rcx,
                    registers.rdx,
                    unsafe { vmx::vmread(vmcs::guest::CR3).unwrap_or(0) }
                );
            }
        }

        ExitReason::CrAccess => exit_handler::handle_cr_access(registers),
        ExitReason::RdMsr => exit_handler::handle_msr_read(registers),
        ExitReason::WrMsr => exit_handler::handle_msr_write(registers),
        ExitReason::CpuId => exit_handler::handle_cpuid(registers),

        ExitReason::Invd => {
            unsafe { asm!("invd") };
        }

        ExitReason::EptViolation | ExitReason::EptMisconfigure => {
            unsafe { asm!("int 3") };
            unreachable!("EPT not implemented!");
        }

        reason => {
            log::error!("Unknown VMEXIT reason: {:?}", reason);
            unsafe { asm!("int 3") };
        }
    };

    if state.vmx_root_incr_rip {
        /* Skip ahead to the next instruction */
        unsafe {
            let guest_rip = vmx::vmread(vmcs::guest::RIP).unwrap_or(0);
            let inst_length = vmx::vmread(vmcs::ro::VMEXIT_INSTRUCTION_LEN).unwrap_or(0);
            let _ = vmx::vmwrite(vmcs::guest::RIP, guest_rip + inst_length);
        }
    }

    state.vmx_root_mode = false;
    state.vm_exit_scheduled
}

extern "system" fn callback_vmresume_failed() {
    /* we only reach this point if resume fails */
    let error = unsafe { vmx::vmread(VM_INSTRUCTION_ERROR) };
    panic!("VMRESUME failed: {:?}", error);
}

extern "system" fn callback_vmoff_execute(guest_registers: &mut CpuRegisters) {
    let state = cpu_states::current();
    state.vmx_root_mode = true;

    /*
     * Save rip and rsp for exiting.
     *
     * Note:
     * The rip should already be advanced past the exit causing instruction.
     */
    let guest_rsp = guest_registers.rsp();
    let guest_rip = guest_registers.rip();

    crate::vmx::disable_current_cpu();
    state.vmx_root_mode = false;

    /* restore the cr3 */
    unsafe {
        controlregs::cr3_write(guest_registers.cr3());
    }
    unsafe {
        asm!(
            /* Setup the exit RSP (+8 for the return RIP) */
            "mov rax, {exit_rsp}",
            "sub rax, 08h",
            "mov [rax], {exit_rip}",

            /*
             * Set RSP to the registers and append the return rsp at the end
             * Note:
             * The VMExitHandler does allocate us an extra byte so we actually can put the exit rsp here.
             */
            "mov rsp, {registers}",
            "mov [rsp + 17 * 8], rax",

            /* Same order as CpuRegisters */
            "popfq",
            "pop rax",
            "pop rcx",
            "pop rdx",
            "pop rbx",
            "pop rbp", /* the stack ptr */
            "pop rbp",
            "pop rsi",
            "pop rdi",
            "pop r8",
            "pop r9",
            "pop r10",
            "pop r11",
            "pop r12",
            "pop r13",
            "pop r14",
            "pop r15",

            "movaps xmm5, [rsp + 8 + 0 * 16]",
            "movaps xmm4, [rsp + 8 + 1 * 16]",
            "movaps xmm3, [rsp + 8 + 2 * 16]",
            "movaps xmm2, [rsp + 8 + 3 * 16]",
            "movaps xmm1, [rsp + 8 + 4 * 16]",
            "movaps xmm0, [rsp + 8 + 5 * 16]",

            /* the new rsp has been set above */
            "pop rsp",
            "ret",

            registers = in(reg) guest_registers,
            exit_rsp = in(reg) guest_rsp,
            exit_rip = in(reg) guest_rip,
            options(noreturn)
        );
    }
}
