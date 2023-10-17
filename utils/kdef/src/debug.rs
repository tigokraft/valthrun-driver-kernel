//! Debugger support.

/// `DbgPrintEx` Message severity.
#[repr(C)]
pub enum DPFLTR_LEVEL {
    ERROR = 0,
    WARNING,
    TRACE,
    INFO,
}
