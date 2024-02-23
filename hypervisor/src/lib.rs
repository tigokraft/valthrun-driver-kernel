#![no_std]
#![feature(core_intrinsics)]
#![feature(naked_functions)]
#![allow(internal_features)]
#![feature(sync_unsafe_cell)]
#![feature(pointer_is_aligned)]
#![feature(new_uninit)]
#![feature(asm_const)]

use core::arch::asm;

use kapi::{
    KeLowerIrql,
    KeRaiseIrql,
    DISPATCH_LEVEL,
};
use logger::create_app_logger;
use obfstr::obfstr;
use panic_hook::DEBUG_IMPORTS;
use utils_imports::provider::SystemExport;
use winapi::{
    km::wdm::DRIVER_OBJECT,
    shared::{
        ntdef::{
            NTSTATUS,
            UNICODE_STRING,
        },
        ntstatus::{
            STATUS_DRIVER_INTERNAL_ERROR,
            STATUS_SUCCESS,
        },
    },
};

extern crate alloc;

mod cpu;
mod cpu_states;
mod logger;
mod logging;
mod mem;
mod msr;
mod panic_hook;
mod processor;
mod utils;
mod vm;
mod vmx;

extern "system" fn driver_unload(_driver: &mut DRIVER_OBJECT) {
    log::info!("Unloading...");

    /* Disable current virtualization is we're within a VM */
    vm::exit_virtualisation();

    /* Disable virtualization in its entirely (in case vmxon is set but we're not in a VM) */
    processor::run_on_all(|| vmx::disable_current_cpu());

    //cpu_states::free(); FIXME: Reenable this, only deactivated to debug VM exits
    log::info!("Unloaded");
}

#[no_mangle]
pub extern "system" fn driver_entry(
    driver: *mut DRIVER_OBJECT,
    _registry_path: *const UNICODE_STRING,
) -> NTSTATUS {
    SystemExport::initialize(None);
    if DEBUG_IMPORTS.resolve().is_err() {
        return STATUS_DRIVER_INTERNAL_ERROR;
    }

    {
        log::set_max_level(log::LevelFilter::Trace);
        let logger = create_app_logger();
        let _ = log::set_logger(logger);

        let (message_buffer, record_buffer) = {
            let queue = logger.queue().lock();
            (
                queue.message_buffer().as_ptr() as u64,
                queue.record_buffer().as_ptr() as u64,
            )
        };
        log::debug!(
            "Log message_buffer = 0x{:X}, record_buffer = 0x{:X}",
            message_buffer,
            record_buffer
        );
    }

    let driver = match unsafe { driver.as_mut() } {
        Some(driver) => driver,
        None => {
            log::error!("Manual mapping is not yet supported.");
            return STATUS_DRIVER_INTERNAL_ERROR;
        }
    };

    if let Err(error) = kapi::initialize(Some(driver)) {
        log::error!("{}: {:#}", obfstr!("kapi failed to initialize"), error);
        return STATUS_DRIVER_INTERNAL_ERROR;
    }

    let irql = KeRaiseIrql(DISPATCH_LEVEL);
    driver.DriverUnload = Some(driver_unload);
    KeLowerIrql(irql);
    if let Err(error) = rust_driver_entry(driver) {
        log::error!("{}: {:#}", obfstr!("driver init failed"), error);
        //unsafe { asm!("int 3") };
        driver_unload(driver);
        return STATUS_DRIVER_INTERNAL_ERROR;
    }

    log::debug!("Status success");
    STATUS_SUCCESS
}

fn rust_driver_entry(_driver: &mut DRIVER_OBJECT) -> anyhow::Result<()> {
    log::info!("{}", obfstr!("Loading Hypervisor"));
    log::info!("  CPU: {}", cpu::name());
    log::info!("  VMX: {:?}", vmx::feature_support());

    cpu_states::allocate()?;
    log::debug!("{}", obfstr!("CPU states allocated"));

    log::debug!("Before states:");
    log::debug!("  Hypervisor ID: {:X?}", cpu::hypervisor_id());
    log::debug!("  Processor ID: {:X?}", processor::current());

    vm::virtualize_current_system()?;
    log::debug!("{}", obfstr!("Current system virtualized"));

    log::debug!("After states");
    log::debug!("  Hypervisor ID: {:X?}", cpu::hypervisor_id());

    unsafe { asm!("int 3") };
    // vm::exit_virtualisation();
    // log::debug!("Cleanup states");
    // log::debug!("  Hypervisor ID: {:X?}", cpu::hypervisor_id());
    Ok(())
}
