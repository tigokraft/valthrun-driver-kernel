#![no_std]
#![allow(internal_features)]
#![feature(core_intrinsics)]
#![feature(naked_functions)]

use entry::FnDriverEntry;
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
            STATUS_SUCCESS,
        },
    },
};

use crate::imports::LL_GLOBAL_IMPORTS;

extern crate alloc;

mod entry;
mod imports;
mod logger;
mod panic_hook;

#[no_mangle]
pub extern "system" fn driver_entry(
    entry_arg1: *mut DRIVER_OBJECT,
    entry_arg2: *const UNICODE_STRING,
    entry_point: FnDriverEntry,
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

    if entry::has_custom_entry() {
        log::debug!(
            "{}",
            obfstr!("Restoring original entry & calling original entry")
        );
        if let Err(err) = entry::restore_original_entry(entry_point) {
            log::error!("{}: {:?}", obfstr!("Failed to restore entry point"), err);
            return STATUS_FAILED_DRIVER_ENTRY;
        }

        {
            let status = entry_point(entry_arg1, entry_arg2);
            if !status.is_ok() {
                log::debug!(
                    "{}: {}",
                    obfstr!("Original driver returned non zero status code"),
                    status
                );
                return status;
            }
        }
    } else {
        log::debug!("{}", obfstr!("No custom entry. Do not patch entry point."));
    }

    let ll_imports = match LL_GLOBAL_IMPORTS.resolve() {
        Ok(imports) => imports,
        Err(error) => {
            log::error!(
                "{}: {:#}",
                obfstr!("Failed to initialize ll imports"),
                error
            );
            return STATUS_FAILED_DRIVER_ENTRY;
        }
    };

    log::info!("{}", obfstr!("Manually mapped driver via UEFI."));
    log::debug!(
        "  {} {:X}.",
        obfstr!("System range start is"),
        ll_imports.MmSystemRangeStart as u64,
    );
    log::debug!("  {} {:X}", obfstr!("IRQL level at"), unsafe {
        (ll_imports.KeGetCurrentIrql)()
    });

    let driver_name = UNICODE_STRING::from_bytes(obfstr::wide!("\\Driver\\valthrun-driver"));
    let result = unsafe {
        (ll_imports.IoCreateDriver)(&driver_name, internal_driver_entry as usize as *const _)
    };
    let status = if let Err(code) = result.ok() {
        log::error!(
            "{} {:X}",
            obfstr!("Failed to create new driver for manually mapped driver:"),
            code
        );

        STATUS_FAILED_DRIVER_ENTRY
    } else {
        STATUS_SUCCESS
    };

    status
}

extern "C" fn internal_driver_entry(
    driver: &mut DRIVER_OBJECT,
    registry_path: *const UNICODE_STRING,
) -> NTSTATUS {
    let registry_path = unsafe { registry_path.as_ref() }.map(|path| path.as_string_lossy());
    {
        let registry_path = registry_path
            .as_ref()
            .map(|path| path.as_str())
            .unwrap_or("None");

        log::info!(
            "Initialize UEFI driver at {:X} ({:?}).",
            driver as *mut _ as u64,
            registry_path,
        );
    }

    log::warn!("This is currently only a stub driver without any functionality!");

    STATUS_SUCCESS
}
