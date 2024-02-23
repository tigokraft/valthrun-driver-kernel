use core::arch::asm;

use winapi::shared::ntdef::KIRQL;

pub const PASSIVE_LEVEL: KIRQL = 0; // Passive release level
pub const LOW_LEVEL: KIRQL = 0; // Lowest interrupt level
pub const APC_LEVEL: KIRQL = 1; // APC interrupt level
pub const DISPATCH_LEVEL: KIRQL = 2; // Dispatcher level
pub const CMCI_LEVEL: KIRQL = 5; // CMCI handler level

pub const CLOCK_LEVEL: KIRQL = 13; // Interval clock level
pub const IPI_LEVEL: KIRQL = 14; // Interprocessor interrupt level
pub const DRS_LEVEL: KIRQL = 14; // Deferred Recovery Service level
pub const POWER_LEVEL: KIRQL = 14; // Power failure level
pub const PROFILE_LEVEL: KIRQL = 15; // timer used for profiling.
pub const HIGH_LEVEL: KIRQL = 15; // Highest interrupt level

#[allow(non_snake_case)]
pub fn KeGetCurrentIrql() -> KIRQL {
    let ret: i64;
    unsafe { asm!("mov {:r}, cr8", out(reg) ret) };
    ret as KIRQL
}

#[allow(non_snake_case)]
pub fn KeLowerIrql(target: KIRQL) {
    let value = target as i64;
    unsafe { asm!("mov cr8, {:r}", in(reg) value) };
}

#[allow(non_snake_case)]
pub fn KeRaiseIrql(target: KIRQL) -> KIRQL {
    let current_irql = KeGetCurrentIrql();

    let value = target as i64;
    unsafe { asm!("mov cr8, {:r}", in(reg) value) };

    current_irql
}
