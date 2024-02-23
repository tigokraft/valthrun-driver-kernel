use x86::{
    self,
    controlregs::{
        self,
        Cr0,
        Cr4,
    },
    current::rflags::{
        self,
        RFlags,
    },
    dtables::{
        self,
        DescriptorTablePointer,
    },
    msr::{
        self,
        IA32_EFER,
        IA32_FS_BASE,
        IA32_GS_BASE,
        IA32_SYSENTER_CS,
        IA32_SYSENTER_EIP,
        IA32_SYSENTER_ESP,
    },
    segmentation::{
        self,
        Descriptor,
        SegmentSelector,
    },
    task,
    vmx::vmcs::{
        self,
        guest::IA32_DEBUGCTL_FULL,
    },
};

use crate::{
    utils,
    vmx::vm_write,
};

pub struct VmGuestConfiguration {
    pub gdt: DescriptorTablePointer<Descriptor>,
    pub idt: DescriptorTablePointer<Descriptor>,

    pub es: SegmentSelector,
    pub cs: SegmentSelector,
    pub ss: SegmentSelector,
    pub ds: SegmentSelector,
    pub fs: SegmentSelector,
    pub gs: SegmentSelector,
    pub tr: SegmentSelector,
    pub ldtr: SegmentSelector,

    pub cr0: Cr0,
    pub cr3: u64,
    pub cr4: Cr4,

    pub fs_base: u64,
    pub gs_base: u64,

    pub rsp: u64,
    pub rip: u64,
    pub rflags: RFlags,

    pub sysenter_cs: u64,
    pub sysenter_esp: u64,
    pub sysenter_eip: u64,
    pub efer: u64,
}

impl VmGuestConfiguration {
    pub fn from_current_host(rip: u64, rsp: u64) -> Self {
        let mut gdt = DescriptorTablePointer::default();
        unsafe { dtables::sgdt(&mut gdt) };

        let mut idt = DescriptorTablePointer::default();
        unsafe { dtables::sidt(&mut idt) };

        unsafe {
            Self {
                gdt,
                idt,

                cs: segmentation::cs(),
                ds: segmentation::ds(),
                es: segmentation::es(),
                fs: segmentation::fs(),
                gs: segmentation::gs(),
                ss: segmentation::ss(),
                tr: task::tr(),
                ldtr: dtables::ldtr(),

                fs_base: msr::rdmsr(IA32_FS_BASE),
                gs_base: msr::rdmsr(IA32_GS_BASE),

                cr0: controlregs::cr0(),
                cr3: controlregs::cr3(),
                cr4: controlregs::cr4(),

                rflags: rflags::read(),

                sysenter_cs: msr::rdmsr(IA32_SYSENTER_CS),
                sysenter_eip: msr::rdmsr(IA32_SYSENTER_EIP),
                sysenter_esp: msr::rdmsr(IA32_SYSENTER_ESP),
                efer: msr::rdmsr(IA32_EFER),

                rip,
                rsp,
            }
        }
    }

    pub fn apply(&self) -> anyhow::Result<()> {
        macro_rules! setup_selector {
            ($selector:path, $table:expr, $variable:expr) => {{
                paste::paste! {
                    vm_write!(vmcs::guest::[< $selector _ SELECTOR >], ($variable).bits() as u64)?;
                    vm_write!(vmcs::guest::[< $selector _ ACCESS_RIGHTS >], utils::get_segment_access_right($table, $variable) as u64)?;
                    vm_write!(vmcs::guest::[< $selector _ LIMIT >], utils::get_segment_limit($table, $variable) as u64)?;
                    vm_write!(vmcs::guest::[< $selector _ BASE >], utils::get_segment_base($table, $variable) as u64)?;
                }
            }};
        }

        unsafe {
            vm_write!(
                vmcs::guest::IA32_DEBUGCTL_FULL,
                msr::rdmsr(IA32_DEBUGCTL_FULL) & 0xFFFFFFFF
            )?;
            vm_write!(
                vmcs::guest::IA32_DEBUGCTL_HIGH,
                msr::rdmsr(IA32_DEBUGCTL_FULL) >> 32
            )?;

            setup_selector!(CS, &self.gdt, self.cs);
            setup_selector!(SS, &self.gdt, self.ss);
            setup_selector!(DS, &self.gdt, self.ds);
            setup_selector!(ES, &self.gdt, self.es);
            setup_selector!(FS, &self.gdt, self.fs);
            setup_selector!(GS, &self.gdt, self.gs);
            setup_selector!(TR, &self.gdt, self.tr);
            setup_selector!(LDTR, &self.gdt, self.ldtr);

            vm_write!(vmcs::guest::FS_BASE, self.fs_base)?;
            vm_write!(vmcs::guest::GS_BASE, self.gs_base)?;

            vm_write!(vmcs::guest::GDTR_BASE, self.gdt.base as u64)?;
            vm_write!(vmcs::guest::GDTR_LIMIT, self.gdt.limit as u64)?;

            vm_write!(vmcs::guest::IDTR_BASE, self.idt.base as u64)?;
            vm_write!(vmcs::guest::IDTR_LIMIT, self.idt.limit as u64)?;

            vm_write!(vmcs::guest::IA32_SYSENTER_CS, self.sysenter_cs)?;
            vm_write!(vmcs::guest::IA32_SYSENTER_EIP, self.sysenter_eip)?;
            vm_write!(vmcs::guest::IA32_SYSENTER_ESP, self.sysenter_esp)?;
            vm_write!(vmcs::guest::IA32_EFER_FULL, self.efer)?;

            vm_write!(vmcs::guest::DR7, 0x400)?;

            vm_write!(vmcs::guest::CR0, self.cr0.bits() as u64)?;
            vm_write!(vmcs::guest::CR3, self.cr3)?;
            vm_write!(vmcs::guest::CR4, self.cr4.bits() as u64)?;

            vm_write!(vmcs::guest::RSP, self.rsp)?;
            vm_write!(vmcs::guest::RIP, self.rip)?;
            vm_write!(vmcs::guest::RFLAGS, self.rflags.bits())?;
            vm_write!(vmcs::guest::LINK_PTR_FULL, u64::MAX)?;
        }

        /* TODO: Add support for VMX_PREEMPTION_TIMER_VALUE */

        Ok(())
    }
}
