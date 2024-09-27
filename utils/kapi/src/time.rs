#![allow(non_snake_case)]
use core::{
    ops::Sub,
    time::Duration,
};

use lazy_link::lazy_link;

#[lazy_link(resolver = "kapi_kmodule::resolve_import")]
extern "C" {
    pub fn KeQueryPerformanceCounter(PerformanceFrequency: *mut u64) -> u64;
}

/// A measurement of a monotonically nondecreasing clock.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant {
    /// Value in nanoseconds
    value: u64,
}

impl Instant {
    pub fn now() -> Self {
        let mut frequency = 0;
        let counter = unsafe { KeQueryPerformanceCounter(&mut frequency) };

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
