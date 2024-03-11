#![cfg_attr(not(test), no_std)]
#![feature(core_intrinsics)]
#![feature(naked_functions)]
#![allow(internal_features)]
#![feature(sync_unsafe_cell)]
#![feature(pointer_is_aligned)]
#![feature(new_uninit)]
#![feature(asm_const)]
#![feature(allocator_api)]

use core::arch::asm;

use kapi::{
    KeLowerIrql,
    KeRaiseIrql,
    NonPagedAllocator,
    DISPATCH_LEVEL,
};
use logger::create_app_logger;
use obfstr::obfstr;
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
use x86::controlregs;

extern crate alloc;

mod cpu;
mod cpu_state;
mod ept;
mod logger;
mod logging;
mod mem;
mod msr;
mod processor;
mod utils;
mod vm;
mod vmx;

#[cfg(not(test))]
mod panic_hook;

#[global_allocator]
#[cfg(not(test))]
static GLOBAL_ALLOC: NonPagedAllocator = NonPagedAllocator;

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
    #[cfg(not(test))]
    if !panic_hook::setup_panic_handler() {
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
    let status = rust_driver_entry(driver);
    KeLowerIrql(irql);
    if let Err(error) = status {
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

    cpu_state::allocate()?;
    log::debug!("{}", obfstr!("CPU states allocated"));

    log::debug!("Before states:");
    log::debug!("  Hypervisor ID: {:X?}", cpu::hypervisor_id());
    log::debug!("  Processor ID: {:X?}", processor::current());
    log::debug!("  CR3: {:X}", unsafe { controlregs::cr3() });

    // let mtrr = ept::read_mtrr()?;
    // log::debug!("{:?}", mtrr.capability);
    // for index in 0..mtrr.descriptor_count {
    //     log::debug!(
    //         "{}: {:0>16X} - {:0>16X}: {:?}",
    //         index,
    //         mtrr.descriptors[index].base_address,
    //         mtrr.descriptors[index].length,
    //         mtrr.descriptors[index].memory_type
    //     );
    // }

    vm::virtualize_current_system()?;
    log::debug!("{}", obfstr!("Current system virtualized"));

    log::debug!("After states");
    log::debug!("  Hypervisor ID: {:X?}", cpu::hypervisor_id());
    log::debug!("  CR3: {:X}", unsafe { controlregs::cr3() });

    //unsafe { asm!("int 3") };
    // vm::exit_virtualisation();
    // log::debug!("Cleanup states");
    // log::debug!("  Hypervisor ID: {:X?}", cpu::hypervisor_id());
    Ok(())
}
