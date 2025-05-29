#![no_std]
#![allow(internal_features)]
#![feature(core_intrinsics)]

use alloc::format;

use driver::{
    get_logger_instance,
    metrics::RECORD_TYPE_DRIVER_STATUS,
};
use entry::FnDriverEntry;
use imports::IoCreateDriver;
use kalloc::NonPagedAllocator;
use kapi::{
    thread,
    Instant,
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
            STATUS_SUCCESS,
        },
    },
};

extern crate alloc;

mod entry;
mod imports;
mod panic_hook;

#[global_allocator]
static GLOBAL_ALLOC: NonPagedAllocator = NonPagedAllocator::new(0x123333);

#[no_mangle]
pub extern "system" fn driver_entry(
    entry_arg1: *mut DRIVER_OBJECT,
    entry_arg2: *const UNICODE_STRING,
    entry_point: FnDriverEntry,
) -> NTSTATUS {
    utils_kernelbase::initialize(None);

    log::set_max_level(LevelFilter::Trace);
    let _ = log::set_logger(get_logger_instance());

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

    log::info!("{}", obfstr!("Manually mapped driver via UEFI."));
    thread::spawn(|| {
        log::debug!("Waiting for the system to boot up before initializing");

        let now = Instant::now();
        /* Lets wait a little bit until WSK is ready, else the driver init will fail :( */
        thread::sleep_ms(25_000);
        log::debug!("Elapsed: {:#?}", now.elapsed());

        let driver_name = UNICODE_STRING::from_bytes(obfstr::wide!("\\Driver\\valthrun-driver"));
        let result = unsafe { IoCreateDriver(&driver_name, internal_driver_entry as *const _) };
        if let Err(code) = result.ok() {
            log::error!(
                "{} {:X}",
                obfstr!("Failed to create new driver for UEFI driver:"),
                code
            );
        };

        if let Some(metrics) = driver::metrics_client() {
            /* report the load result if metrics could be already initialized */
            metrics.add_record(
                RECORD_TYPE_DRIVER_STATUS,
                format!(
                    "load:{:X}, version:{}, type:{}",
                    result,
                    env!("CARGO_PKG_VERSION"),
                    "uefi"
                ),
            );
        }
    });

    STATUS_SUCCESS
}

extern "C" fn internal_driver_entry(
    driver: &mut DRIVER_OBJECT,
    registry_path: *const UNICODE_STRING,
) -> NTSTATUS {
    {
        let registry_path = unsafe { registry_path.as_ref() }.map(|path| path.as_string_lossy());
        let registry_path = registry_path
            .as_ref()
            .map(|path| path.as_str())
            .unwrap_or("None");

        log::info!(
            "Initialize UEFI driver at {:X} ({:?}). Kernel base: {:X}",
            driver as *mut _ as u64,
            registry_path,
            utils_kernelbase::get().unwrap_or(0)
        );
    }
    driver::internal_driver_entry(unsafe { &mut *(driver as *mut DRIVER_OBJECT) })
}
