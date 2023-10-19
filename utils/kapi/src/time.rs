use core::{
    ops::Sub,
    time::Duration,
};

use utils_imports::{
    dynamic_import_table,
    provider::SystemExport,
};

type KeQueryPerformanceCounter = unsafe extern "C" fn(PerformanceFrequency: *mut u64) -> u64;
type KeQueryTimeIncrement = unsafe extern "C" fn() -> u32;

dynamic_import_table! {
    pub imports TIME_IMPORTS {
        pub KeQueryPerformanceCounter: KeQueryPerformanceCounter = SystemExport::new(obfstr!("KeQueryPerformanceCounter")),
        pub KeQueryTimeIncrement: KeQueryTimeIncrement = SystemExport::new(obfstr!("KeQueryTimeIncrement")),
    }
}

/// A measurement of a monotonically nondecreasing clock.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant {
    /// Value in nanoseconds
    value: u64,
}

impl Instant {
    pub fn new() -> Self {
        let imports = TIME_IMPORTS.unwrap();
        let performance_counter =
            unsafe { (imports.KeQueryPerformanceCounter)(core::ptr::null_mut()) } as u64;
        let time_increment = unsafe { (imports.KeQueryTimeIncrement)() } as u64 * 100;
        Self {
            value: performance_counter * time_increment,
        }
    }
}

impl Sub<Instant> for Instant {
    type Output = Duration;

    fn sub(self, other: Instant) -> Self::Output {
        debug_assert!(self.value >= other.value);
        Duration::from_nanos(self.value - other.value)
    }
}
