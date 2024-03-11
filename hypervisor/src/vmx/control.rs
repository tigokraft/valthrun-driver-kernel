use x86::{
    msr,
    vmx::vmcs::{
        self,
        control::{
            EntryControls,
            ExitControls,
            PinbasedControls,
            PrimaryControls,
            SecondaryControls,
        },
    },
};

/// The types of the control field.
#[derive(Clone, Copy, Debug)]
pub enum VmxControl {
    Primary(PrimaryControls),
    Secondary(SecondaryControls),

    Pinbased(PinbasedControls),
    Entry(EntryControls),
    Exit(ExitControls),
}

impl VmxControl {
    pub fn vmcs_field(&self) -> u32 {
        match self {
            Self::Entry(_) => vmcs::control::VMENTRY_CONTROLS,
            Self::Exit(_) => vmcs::control::VMEXIT_CONTROLS,
            Self::Pinbased(_) => vmcs::control::PINBASED_EXEC_CONTROLS,

            Self::Primary(_) => vmcs::control::PRIMARY_PROCBASED_EXEC_CONTROLS,
            Self::Secondary(_) => vmcs::control::SECONDARY_PROCBASED_EXEC_CONTROLS,
        }
    }

    pub fn value(&self) -> u32 {
        match self {
            Self::Entry(value) => value.bits(),
            Self::Exit(value) => value.bits(),
            Self::Pinbased(value) => value.bits(),

            Self::Primary(value) => value.bits(),
            Self::Secondary(value) => value.bits(),
        }
    }

    /// Returns an adjust value for the control field according to the capability
    /// MSR.
    pub fn adjusted_value(&self) -> u64 {
        // Each bit of the following VMCS values might have to be set or cleared
        // according to the value indicated by the VMX capability MSRs.
        //  - pin-based VM-execution controls,
        //  - primary processor-based VM-execution controls,
        //  - secondary processor-based VM-execution controls.
        //
        // The VMX capability MSR is composed of two 32bit values, the lower 32bits
        // indicate bits can be 0, and the higher 32bits indicates bits can be 1.
        // In other words, if those bits are "cleared", corresponding bits MUST BE 1
        // and MUST BE 0 respectively. The below summarizes the interpretation:
        //
        //        Lower bits (allowed 0) Higher bits (allowed 1) Meaning
        // Bit X  1                      1                       The bit X is flexible
        // Bit X  1                      0                       The bit X is fixed to 0
        // Bit X  0                      1                       The bit X is fixed to 1
        //
        // The following code enforces this logic by setting bits that must be 1,
        // and clearing bits that must be 0.
        //
        // See: A.3.1 Pin-Based VM-Execution Controls
        // See: A.3.2 Primary Processor-Based VM-Execution Controls
        // See: A.3.3 Secondary Processor-Based VM-Execution Controls
        let capabilities = unsafe { msr::rdmsr(self.capability_msr()) };
        let allowed0 = capabilities as u32;
        let allowed1 = (capabilities >> 32) as u32;

        let mut effective_value = self.value();
        effective_value |= allowed0;
        effective_value &= allowed1;

        u64::from(effective_value)
    }

    pub fn capability_msr(&self) -> u32 {
        const IA32_VMX_BASIC_VMX_CONTROLS_FLAG: u64 = 1 << 55;

        // This determines the right VMX capability MSR based on the value of
        // IA32_VMX_BASIC. This is required to fullfil the following requirements:
        //
        // "It is necessary for software to consult only one of the capability MSRs
        //  to determine the allowed settings of the pin based VM-execution controls:"
        // See: A.3.1 Pin-Based VM-Execution Controls
        let vmx_basic = unsafe { msr::rdmsr(x86::msr::IA32_VMX_BASIC) };
        let true_cap_msr_supported = (vmx_basic & IA32_VMX_BASIC_VMX_CONTROLS_FLAG) != 0;

        match (self, true) {
            (Self::Primary(_), true) => x86::msr::IA32_VMX_TRUE_PROCBASED_CTLS,
            (Self::Primary(_), false) => x86::msr::IA32_VMX_PROCBASED_CTLS,

            // There is no TRUE MSR for IA32_VMX_PROCBASED_CTLS2. Just use
            // IA32_VMX_PROCBASED_CTLS2 unconditionally.
            (VmxControl::Secondary(_), _) => x86::msr::IA32_VMX_PROCBASED_CTLS2,

            (Self::Pinbased(_), true) => x86::msr::IA32_VMX_TRUE_PINBASED_CTLS,
            (Self::Pinbased(_), false) => x86::msr::IA32_VMX_PINBASED_CTLS,

            (Self::Exit(_), true) => x86::msr::IA32_VMX_TRUE_EXIT_CTLS,
            (Self::Exit(_), false) => x86::msr::IA32_VMX_EXIT_CTLS,
            (Self::Entry(_), true) => x86::msr::IA32_VMX_TRUE_ENTRY_CTLS,
            (Self::Entry(_), false) => x86::msr::IA32_VMX_ENTRY_CTLS,
        }
    }
}

impl From<PrimaryControls> for VmxControl {
    fn from(value: PrimaryControls) -> Self {
        Self::Primary(value)
    }
}

impl From<SecondaryControls> for VmxControl {
    fn from(value: SecondaryControls) -> Self {
        Self::Secondary(value)
    }
}

impl From<PinbasedControls> for VmxControl {
    fn from(value: PinbasedControls) -> Self {
        Self::Pinbased(value)
    }
}

impl From<ExitControls> for VmxControl {
    fn from(value: ExitControls) -> Self {
        Self::Exit(value)
    }
}

impl From<EntryControls> for VmxControl {
    fn from(value: EntryControls) -> Self {
        Self::Entry(value)
    }
}
