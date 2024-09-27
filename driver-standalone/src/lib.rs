#![no_std]
#![allow(internal_features)]
#![feature(core_intrinsics)]
#![feature(naked_functions)]

use alloc::format;

use driver::{
    get_logger_instance,
    metrics::RECORD_TYPE_DRIVER_STATUS,
    status::{
        CSTATUS_DRIVER_ALREADY_LOADED,
        CSTATUS_DRIVER_PREINIT_FAILED,
    },
};
use imports::IoCreateDriver;
use kalloc::NonPagedAllocator;
use kapi::{
    NTStatusEx,
    UnicodeStringEx,
};
use log::LevelFilter;
use obfstr::obfstr;
use winapi::{
    km::wdm::DRIVER_OBJECT,
    shared::{
        ntdef::{
            NTSTATUS,
            UNICODE_STRING,
        },
        ntstatus::{
            STATUS_FAILED_DRIVER_ENTRY,
            STATUS_OBJECT_NAME_COLLISION,
            STATUS_SUCCESS,
        },
    },
};

extern crate alloc;

mod imports;
mod panic_hook;

#[global_allocator]
static GLOBAL_ALLOC: NonPagedAllocator = NonPagedAllocator::new(0x123333);

#[no_mangle]
pub extern "system" fn driver_entry(
    driver: *mut DRIVER_OBJECT,
    registry_path: *const UNICODE_STRING,
) -> NTSTATUS {
    utils_kernelbase::initialize(None);

    log::set_max_level(LevelFilter::Trace);
    let _ = log::set_logger(get_logger_instance());

    if let Err(err) = kapi::initialize(None) {
        log::error!("{}: {:?}", "Failed to initialize kernel API", err);
        return STATUS_FAILED_DRIVER_ENTRY;
    }

    let status = match unsafe { driver.as_mut() } {
        Some(driver) => internal_driver_entry(driver, registry_path),
        None => {
            log::info!("{}", obfstr!("Manually mapped driver."));

            // TODO(low): May improve hiding via:
            // https://research.checkpoint.com/2021/a-deep-dive-into-doublefeature-equation-groups-post-exploitation-dashboard/
            let driver_name =
                UNICODE_STRING::from_bytes(obfstr::wide!("\\Driver\\valthrun-driver"));
            let result = unsafe { IoCreateDriver(&driver_name, internal_driver_entry as *const _) };
            if let Err(code) = result.ok() {
                if code == STATUS_OBJECT_NAME_COLLISION {
                    log::error!("{}", obfstr!("Failed to create valthrun driver as a driver with this name is already loaded."));
                    CSTATUS_DRIVER_ALREADY_LOADED
                } else {
                    log::error!(
                        "{} {:X}",
                        obfstr!("Failed to create new driver for manually mapped driver:"),
                        code
                    );
                    CSTATUS_DRIVER_PREINIT_FAILED
                }
            } else {
                STATUS_SUCCESS
            }
        }
    };

    if let Some(metrics) = driver::metrics_client() {
        /* report the load result if metrics could be already initialized */
        metrics.add_record(
            RECORD_TYPE_DRIVER_STATUS,
            format!(
                "load:{:X}, version:{}, type:{}",
                status,
                env!("CARGO_PKG_VERSION"),
                if driver.is_null() {
                    "manual-mapped"
                } else {
                    "service"
                }
            ),
        );
    }

    if status != STATUS_SUCCESS {
        /* cleanup all pending / initialized resources */
        driver::driver_unload();
    }

    status
}

pub extern "system" fn driver_unload(_self: &mut DRIVER_OBJECT) {
    driver::driver_unload();
}

extern "C" fn internal_driver_entry(
    driver: &mut DRIVER_OBJECT,
    registry_path: *const UNICODE_STRING,
) -> NTSTATUS {
    driver.DriverUnload = Some(driver_unload);

    {
        let registry_path = unsafe { registry_path.as_ref() }.map(|path| path.as_string_lossy());
        let registry_path = registry_path
            .as_ref()
            .map(|path| path.as_str())
            .unwrap_or("None");

        log::info!(
            "Initialize driver at {:X} ({:?}). Kernel base {:X}",
            driver as *mut _ as u64,
            registry_path,
            utils_kernelbase::get().unwrap_or(0)
        );
    }

    driver::internal_driver_entry(driver)
}
