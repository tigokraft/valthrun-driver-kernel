use core::ops::BitAnd;

use kapi::{
    KeGetCurrentIrql,
    DISPATCH_LEVEL,
};
use obfstr::obfstr;
use x86::{
    controlregs::{
        self,
        Cr4,
    },
    cpuid::cpuid,
    current::vmx,
    msr::{
        self,
        rdmsr,
        IA32_FEATURE_CONTROL,
    },
    vmx::vmcs,
};

use crate::{
    cpu_state,
    mem::MemoryAddressEx,
    msr::Ia32FeatureControlMsr,
};

mod control;
pub use control::*;

mod exit;
pub use exit::*;

mod guest_config;
pub use guest_config::*;
/* reexport vmx intrinsics */
pub use x86::current::vmx::*;

/// The region of memory that the logical processor uses to support VMX
/// operation.
///
/// See: 25.11.5 VMXON Region
#[repr(C, align(4096))]
pub struct Vmxon {
    pub revision_id: u32,
    pub data: [u8; 4092],
}
const _: () = assert!(core::mem::size_of::<Vmxon>() == 0x1000);

/// The region of memory that the logical processor uses to represent a virtual
/// CPU. Called virtual-machine control data structure (VMCS).
///
/// See: 25.2 FORMAT OF THE VMCS REGION
#[repr(C, align(4096))]
pub struct Vmcs {
    pub revision_id: u32,
    pub abort_indicator: u32,
    pub data: [u8; 4088],
}

const _: () = assert!(core::mem::size_of::<Vmcs>() == 0x1000);

#[repr(C, align(4096))]
pub struct MsrBitmap {
    pub data: [u8; 4096],
}
const _: () = assert!(core::mem::size_of::<MsrBitmap>() == 0x1000);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum VmxSupport {
    Supported,
    BiosLocked,
    CpuUnsupported,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CpuRegisters {
    pub rflags: u64,

    pub rax: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rbx: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    /* _dummy: u64 /* placeholder for the exit rsp */ */

    /* pub xmm: u128[6], */
}

impl CpuRegisters {
    // /// Save the current CPU registers except for RIP and RSP
    // #[naked]
    // pub extern "cdecl" fn save(&mut self) {
    //     /* TODO! */
    //     unsafe { asm!("ret", options(noreturn)) }
    // }

    // /// Restore all current CPU registers except for RIP and RSP
    // #[naked]
    // pub extern "cdecl" fn restore(&mut self) {
    //     /* TODO! */
    //     unsafe { asm!("ret", options(noreturn)) }
    // }

    pub fn rip(&self) -> u64 {
        unsafe { vmx::vmread(vmcs::guest::RIP).unwrap_or(0) }
    }

    pub fn rsp(&self) -> u64 {
        unsafe { vmx::vmread(vmcs::guest::RSP).unwrap_or(0) }
    }

    pub fn set_rsp(&mut self, value: u64) {
        unsafe {
            let _ = vmx::vmwrite(vmcs::guest::RSP, value);
        }
    }

    pub fn cr3(&self) -> u64 {
        unsafe { vmx::vmread(vmcs::guest::CR3).unwrap_or(0) }
    }

    pub fn read_gp_register(&self, register: GeneralPurposeRegister) -> u64 {
        match register {
            GeneralPurposeRegister::RAX => self.rax,
            GeneralPurposeRegister::RCX => self.rcx,
            GeneralPurposeRegister::RDX => self.rdx,
            GeneralPurposeRegister::RBX => self.rbx,
            GeneralPurposeRegister::RSP => self.rsp(),
            GeneralPurposeRegister::RBP => self.rbp,
            GeneralPurposeRegister::RSI => self.rsi,
            GeneralPurposeRegister::RDI => self.rdi,
            GeneralPurposeRegister::R8 => self.r8,
            GeneralPurposeRegister::R9 => self.r9,
            GeneralPurposeRegister::R10 => self.r10,
            GeneralPurposeRegister::R11 => self.r11,
            GeneralPurposeRegister::R12 => self.r12,
            GeneralPurposeRegister::R13 => self.r13,
            GeneralPurposeRegister::R14 => self.r14,
            GeneralPurposeRegister::R15 => self.r15,
        }
    }

    pub fn write_gp_register(&mut self, register: GeneralPurposeRegister, value: u64) {
        match register {
            GeneralPurposeRegister::RAX => self.rax = value,
            GeneralPurposeRegister::RCX => self.rcx = value,
            GeneralPurposeRegister::RDX => self.rdx = value,
            GeneralPurposeRegister::RBX => self.rbx = value,
            GeneralPurposeRegister::RSP => self.set_rsp(value),
            GeneralPurposeRegister::RBP => self.rbp = value,
            GeneralPurposeRegister::RSI => self.rsi = value,
            GeneralPurposeRegister::RDI => self.rdi = value,
            GeneralPurposeRegister::R8 => self.r8 = value,
            GeneralPurposeRegister::R9 => self.r9 = value,
            GeneralPurposeRegister::R10 => self.r10 = value,
            GeneralPurposeRegister::R11 => self.r11 = value,
            GeneralPurposeRegister::R12 => self.r12 = value,
            GeneralPurposeRegister::R13 => self.r13 = value,
            GeneralPurposeRegister::R14 => self.r14 = value,
            GeneralPurposeRegister::R15 => self.r15 = value,
        }
    }
}

pub fn feature_support() -> VmxSupport {
    let cpuid = cpuid!(1);
    if cpuid.ecx.bitand(1 << 5) == 0 {
        return VmxSupport::CpuUnsupported;
    }

    let features = unsafe { Ia32FeatureControlMsr::from(x86::msr::rdmsr(IA32_FEATURE_CONTROL)) };
    if features.locked() && !features.enabled_vmxon() {
        VmxSupport::BiosLocked
    } else {
        VmxSupport::Supported
    }
}

pub fn feature_enable() -> anyhow::Result<()> {
    let features = unsafe { Ia32FeatureControlMsr::from(x86::msr::rdmsr(IA32_FEATURE_CONTROL)) };
    if features.enabled_vmxon() {
        return Ok(());
    }

    if features.locked() {
        anyhow::bail!("can not enable VMXON feature as it's locked")
    }

    log::debug!("Enabling and locking VMXON feature.");
    let features = features.with_locked(true).with_enabled_vmxon(true).into();
    unsafe { msr::wrmsr(IA32_FEATURE_CONTROL, features) };
    Ok(())
}

pub fn enable_current_cpu() -> anyhow::Result<()> {
    assert!(KeGetCurrentIrql() >= DISPATCH_LEVEL);
    let state = cpu_state::current();
    if state.vmxon_active {
        /* vmx is already acive */
        return Ok(());
    }

    let original_cr4 = unsafe { controlregs::cr4() };
    let original_cr0 = unsafe { controlregs::cr0() };

    // Enable VMX, which allows execution of the VMXON instruction.
    //
    // "Before system software can enter VMX operation, it enables VMX by
    //  setting CR4.VMXE[bit 13] = 1."
    // See: 24.7 ENABLING AND ENTERING VMX OPERATION
    unsafe {
        controlregs::cr4_write(controlregs::cr4() | Cr4::CR4_ENABLE_VMX);
    }

    // In order to enter VMX operation, some bits in CR0 (and CR4) have to be
    // set or cleared as indicated by the FIXED0 and FIXED1 MSRs. The rule is
    // summarized as below (taking CR0 as an example):
    //
    //        IA32_VMX_CR0_FIXED0 IA32_VMX_CR0_FIXED1 Meaning
    // Bit X  1                   (Always 1)          The bit X of CR0 is fixed to 1
    // Bit X  0                   1                   The bit X of CR0 is flexible
    // Bit X  (Always 0)          0                   The bit X of CR0 is fixed to 0
    //
    // Some UEFI implementations do not fullfil those requirements for CR0 and
    // need adjustments.
    //
    // See: A.7 VMX-FIXED BITS IN CR0
    // See: A.8 VMX-FIXED BITS IN CR4
    unsafe {
        let mut cr0 = controlregs::cr0().bits() as u64;
        cr0 |= rdmsr(x86::msr::IA32_VMX_CR0_FIXED0);
        cr0 &= rdmsr(x86::msr::IA32_VMX_CR0_FIXED1);
        controlregs::cr0_write(controlregs::Cr0::from_bits_truncate(cr0 as usize));

        let mut cr4 = controlregs::cr4().bits() as u64;
        cr4 |= rdmsr(x86::msr::IA32_VMX_CR4_FIXED0);
        cr4 &= rdmsr(x86::msr::IA32_VMX_CR4_FIXED1);
        controlregs::cr4_write(controlregs::Cr4::from_bits_truncate(cr4 as usize));
    }

    let result = unsafe { vmx::vmxon(state.vmxon.get_physical_address() as u64) };
    match result {
        Ok(_) => {
            state.vmxon_active = true;
            log::debug!("{} VMX enabled", state.processor_index);
            Ok(())
        }
        Err(err) => {
            /* Restore original control register states */
            unsafe {
                controlregs::cr0_write(original_cr0);
                controlregs::cr4_write(original_cr4);
            }

            anyhow::bail!("{}: {:?}", obfstr!("VMXON"), err)
        }
    }
}

/// Disable VMX for the current CPU
pub fn disable_current_cpu() {
    let state = match cpu_state::try_current() {
        Some(state) => state,
        None => return,
    };

    if !state.vmxon_active {
        /* VMX is not enabled */
        return;
    }

    let result = unsafe { vmx::vmxoff() };
    if let Err(err) = result {
        log::error!(
            "{} {}: {:?}",
            state.processor_index,
            obfstr!("VMOFF failed"),
            err
        );
        return;
    }

    state.vmxon_active = false;
    log::debug!("{} {}", state.processor_index, obfstr!("VMOFF succeeded"));

    /* Disable VMX in CR4 */
    unsafe {
        let mut cr4 = controlregs::cr4();
        cr4 &= !Cr4::CR4_ENABLE_VMX;
        controlregs::cr4_write(cr4);
    }
}

macro_rules! vm_write {
    ($field:expr, $value:expr) => {
        x86::current::vmx::vmwrite($field, $value as u64).map_err(|err| {
            anyhow::anyhow!(
                "{} {}: {:?}",
                obfstr::obfstr!("vmwrite"),
                obfstr::obfstr!(stringify!($field)),
                err
            )
        })
    };
}

pub(crate) use vm_write;
