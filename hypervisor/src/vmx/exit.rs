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
