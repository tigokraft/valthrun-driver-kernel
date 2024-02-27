use core::{
    ops::Sub,
    time::Duration,
};

use utils_imports::{
    dynamic_import_table,
    provider::SystemExport,
};

type KeQueryPerformanceCounter = unsafe extern "C" fn(PerformanceFrequency: *mut u64) -> u64;

dynamic_import_table! {
    pub imports TIME_IMPORTS {
        pub KeQueryPerformanceCounter: KeQueryPerformanceCounter = SystemExport::new(obfstr!("KeQueryPerformanceCounter")),
    }
}

/// A measurement of a monotonically nondecreasing clock.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant {
    /// Value in nanoseconds
    value: u64,
}

impl Instant {
    pub fn now() -> Self {
        let imports = TIME_IMPORTS.unwrap();

        let mut frequency = 0;
        let counter = unsafe { (imports.KeQueryPerformanceCounter)(&mut frequency) };

        Self {
            value: (counter * 1_000_000_000) / frequency,
        }
    }

    pub fn elapsed(&self) -> Duration {
        Instant::now() - *self
    }
}

impl Sub<Instant> for Instant {
    type Output = Duration;

    fn sub(self, other: Instant) -> Self::Output {
        debug_assert!(self.value >= other.value);
        Duration::from_nanos(self.value - other.value)
    }
}
