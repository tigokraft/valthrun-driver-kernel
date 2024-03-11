use bitfield_struct::bitfield;
use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Debug)]
    pub struct ExitReasonFlags: u64 {
        const ENCLAVE_MODE =        1 << 27;
        const PENDING_MTF_EXIT =    1 << 28;
        const EXIT_FROM_ROOT =      1 << 29;
        const VM_ENTRY_FAIL =       1 << 31;
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum ControlRegister {
    Cr0,
    Cr3,
    Cr4,
}

impl ControlRegister {
    pub const fn into_bits(self) -> u64 {
        self as _
    }

    pub const fn from_bits(value: u64) -> Self {
        match value {
            0 => Self::Cr0,
            3 => Self::Cr3,
            4 => Self::Cr4,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum GeneralPurposeRegister {
    RAX,
    RCX,
    RDX,
    RBX,
    RSP,
    RBP,
    RSI,
    RDI,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
}

impl GeneralPurposeRegister {
    pub const fn into_bits(self) -> u64 {
        self as _
    }

    pub const fn from_bits(value: u64) -> Self {
        match value {
            0 => Self::RAX,
            1 => Self::RCX,
            2 => Self::RDX,
            3 => Self::RBX,
            4 => Self::RSP,
            5 => Self::RBP,
            6 => Self::RSI,
            7 => Self::RDI,
            8 => Self::R8,
            9 => Self::R9,
            10 => Self::R10,
            11 => Self::R11,
            12 => Self::R12,
            13 => Self::R13,
            14 => Self::R14,
            15 => Self::R15,
            _ => unreachable!(),
        }
    }
}

#[bitfield(u64)]
pub struct MovCrQualification {
    #[bits(4)]
    pub control_register: ControlRegister,

    #[bits(2)]
    pub access_type: u8,
    pub lmsw_operand_type: bool,

    _reserved1: bool,

    #[bits(4)]
    pub register: GeneralPurposeRegister,

    #[bits(4)]
    _reserved2: u8,

    #[bits(16)]
    pub lmsw_source_data: u16,

    #[bits(32)]
    _reserved3b: u32,
}

// See Table C-1 in Appendix C
#[derive(Clone, Debug)]
pub enum ExitReason {
    NonMaskableInterrupt, /* (VectoredEventInformation) */
    ExternalInterrupt,    /* (VectoredEventInformation) */
    TripleFault,
    InitSignal,
    StartUpIpi,
    IoSystemManagementInterrupt,
    OtherSystemManagementInterrupt,
    InterruptWindow,
    NonMaskableInterruptWindow,
    TaskSwitch,
    CpuId,
    GetSec,
    Hlt,
    Invd,
    InvlPg,
    Rdpmc,
    Rdtsc,
    Rsm,
    VmCall,
    VmClear,
    VmLaunch,
    VmPtrLd,
    VmPtrRst,
    VmRead,
    VmResume,
    VmWrite,
    VmxOff,
    VmxOn,
    CrAccess, /* (CrInformation) */
    MovDr,
    IoInstruction, /* (IoInstructionInformation) */
    RdMsr,
    WrMsr,
    VmEntryInvalidGuestState,
    VmEntryMsrLoad,
    Mwait,
    MonitorTrapFlag,
    Monitor,
    Pause,
    VmEntryMachineCheck,
    TprBelowThreshold,
    ApicAccess, /* (ApicAccessInformation) */
    VirtualEio,
    AccessGdtridtr,
    AccessLdtrTr,
    EptViolation, /* (EptInformation) */
    EptMisconfigure,
    InvEpt,
    Rdtscp,
    VmxPreemptionTimerExpired,
    Invvpid,
    Wbinvd,
    Xsetbv,
    ApicWrite,
    RdRand,
    Invpcid,
    VmFunc,
    Encls,
    RdSeed,
    PageModificationLogFull,
    Xsaves,
    Xrstors,
}

#[derive(Clone, Debug)]
pub struct ExitInformation {
    pub flags: ExitReasonFlags,
    pub reason: ExitReason,
}

impl ExitInformation {
    pub fn from_vmexit_code(reason: u64) -> Option<Self> {
        let basic_reason = (reason & 0x7fff) as u32;
        let flags = ExitReasonFlags::from_bits_truncate(reason);
        let reason = match basic_reason {
            0 => ExitReason::NonMaskableInterrupt,
            1 => ExitReason::ExternalInterrupt,
            2 => ExitReason::TripleFault,
            3 => ExitReason::InitSignal,
            4 => ExitReason::StartUpIpi,
            5 => ExitReason::IoSystemManagementInterrupt,
            6 => ExitReason::OtherSystemManagementInterrupt,
            7 => ExitReason::InterruptWindow,
            8 => ExitReason::NonMaskableInterruptWindow,
            9 => ExitReason::TaskSwitch,
            10 => ExitReason::CpuId,
            11 => ExitReason::GetSec,
            12 => ExitReason::Hlt,
            13 => ExitReason::Invd,
            14 => ExitReason::InvlPg,
            15 => ExitReason::Rdpmc,
            16 => ExitReason::Rdtsc,
            17 => ExitReason::Rsm,
            18 => ExitReason::VmCall,
            19 => ExitReason::VmClear,
            20 => ExitReason::VmLaunch,
            21 => ExitReason::VmPtrLd,
            22 => ExitReason::VmPtrRst,
            23 => ExitReason::VmRead,
            24 => ExitReason::VmResume,
            25 => ExitReason::VmWrite,
            26 => ExitReason::VmxOff,
            27 => ExitReason::VmxOn,
            28 => ExitReason::CrAccess,
            29 => ExitReason::MovDr,
            30 => ExitReason::IoInstruction,
            31 => ExitReason::RdMsr,
            32 => ExitReason::WrMsr,
            33 => ExitReason::VmEntryInvalidGuestState,
            34 => ExitReason::VmEntryMsrLoad,
            // 35 is unused
            36 => ExitReason::Mwait,
            37 => ExitReason::MonitorTrapFlag,
            // 38 is unused
            39 => ExitReason::Monitor,
            40 => ExitReason::Pause,
            41 => ExitReason::VmEntryMachineCheck,
            43 => ExitReason::TprBelowThreshold,
            44 => ExitReason::ApicAccess,
            45 => ExitReason::VirtualEio,
            46 => ExitReason::AccessGdtridtr,
            47 => ExitReason::AccessLdtrTr,
            48 => ExitReason::EptViolation,
            49 => ExitReason::EptMisconfigure,
            50 => ExitReason::InvEpt,
            51 => ExitReason::Rdtscp,
            52 => ExitReason::VmxPreemptionTimerExpired,
            53 => ExitReason::Invvpid,
            54 => ExitReason::Wbinvd,
            55 => ExitReason::Xsetbv,
            56 => ExitReason::ApicWrite,
            57 => ExitReason::RdRand,
            58 => ExitReason::Invpcid,
            59 => ExitReason::VmFunc,
            60 => ExitReason::Encls,
            61 => ExitReason::RdSeed,
            62 => ExitReason::PageModificationLogFull,
            63 => ExitReason::Xsaves,
            64 => ExitReason::Xrstors,
            _reason => return None,
        };

        Some(ExitInformation {
            flags: flags,
            reason,
        })
    }
}
