#![no_std]
#![allow(internal_features)]
#![feature(core_intrinsics)]
#![feature(naked_functions)]

use alloc::format;

use driver::{
    metrics::RECORD_TYPE_DRIVER_STATUS,
    status::{
        CSTATUS_DRIVER_ALREADY_LOADED,
        CSTATUS_DRIVER_PREINIT_FAILED,
    },
};
use kapi::{
    NTStatusEx,
    UnicodeStringEx,
};
use kdef::DPFLTR_LEVEL;
use log::LevelFilter;
use logger::APP_LOGGER;
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
            STATUS_FAILED_DRIVER_ENTRY,
            STATUS_OBJECT_NAME_COLLISION,
            STATUS_SUCCESS,
        },
    },
};

use crate::imports::GLOBAL_IMPORTS;

extern crate alloc;

mod imports;
mod logger;
mod panic_hook;

#[no_mangle]
pub extern "system" fn driver_entry(
    driver: *mut DRIVER_OBJECT,
    registry_path: *const UNICODE_STRING,
) -> NTSTATUS {
    SystemExport::initialize(None);
    if DEBUG_IMPORTS.resolve().is_err() {
        /*
         * If this import fails, we can't do anything else except return an appropiate status code.
         */
        return STATUS_FAILED_DRIVER_ENTRY;
    }

    log::set_max_level(LevelFilter::Trace);
    if log::set_logger(&APP_LOGGER).is_err() {
        let imports = DEBUG_IMPORTS.unwrap();
        unsafe {
            (imports.DbgPrintEx)(
                0,
                DPFLTR_LEVEL::ERROR as u32,
                obfstr!("[VT] Failed to initialize app logger!\n\0").as_ptr(),
            );
        }

        return STATUS_FAILED_DRIVER_ENTRY;
    }

    if let Err(err) = kapi::initialize(None) {
        log::error!("{}: {:?}", "Failed to initialize kernel API", err);
        return STATUS_FAILED_DRIVER_ENTRY;
    }

    let ll_imports = match GLOBAL_IMPORTS.resolve() {
        Ok(imports) => imports,
        Err(error) => {
            log::error!("{}: {:#}", obfstr!("Failed to initialize imports"), error);
            return CSTATUS_DRIVER_PREINIT_FAILED;
        }
    };

    let status = match unsafe { driver.as_mut() } {
        Some(driver) => internal_driver_entry(driver, registry_path),
        None => {
            log::info!("{}", obfstr!("Manually mapped driver."));

            // TODO(low): May improve hiding via:
            // https://research.checkpoint.com/2021/a-deep-dive-into-doublefeature-equation-groups-post-exploitation-dashboard/
            let driver_name =
                UNICODE_STRING::from_bytes(obfstr::wide!("\\Driver\\valthrun-driver"));
            let result = unsafe {
                (ll_imports.IoCreateDriver)(
                    &driver_name,
                    internal_driver_entry as usize as *const _,
                )
            };
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
            SystemExport::kernel_base()
        );
    }

    driver::internal_driver_entry(driver)
}
