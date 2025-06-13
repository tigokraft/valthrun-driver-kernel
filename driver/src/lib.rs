#![no_std]
#![feature(sync_unsafe_cell)]
#![feature(result_flattening)]
#![feature(linkage)]
#![feature(core_intrinsics)]
#![allow(internal_features)]
#![allow(dead_code)]

use alloc::{
    boxed::Box,
    format,
    string::String,
};
use core::{
    cell::SyncUnsafeCell,
    time::Duration,
};

use anyhow::Context;
use device::ValthrunDevice;
use handler::{
    handler_get_modules,
    handler_get_processes,
    HandlerRegistry,
};
use kb::KeyboardInput;
use metrics::{
    MetricsClient,
    MetricsHeartbeat,
};
use mouse::MouseInput;
use obfstr::{
    obfstr,
    obfstring,
};
use status::CSTATUS_DRIVER_INIT_FAILED_PP;
use vtk_wsk::WskInstance;
use winapi::{
    km::wdm::DRIVER_OBJECT,
    shared::{
        ntdef::NTSTATUS,
        ntstatus::STATUS_SUCCESS,
    },
};

use crate::{
    handler::{
        handler_init,
        handler_keyboard_state,
        handler_metrics_record,
        handler_mouse_move,
        handler_protection_toggle,
        handler_read,
        handler_write,
    },
    metrics::RECORD_TYPE_DRIVER_STATUS,
    offsets::initialize_nt_offsets,
    status::{
        CSTATUS_DRIVER_INIT_FAILED,
        CSTATUS_DRIVER_PREINIT_FAILED,
    },
    util::{
        MB_ICONERROR,
        MB_OK,
        MB_SYSTEMMODAL,
    },
    winver::{
        initialize_os_info,
        os_info,
    },
};

mod device;
mod handler;
mod imports;
mod kb;
mod logger;
pub mod metrics;
mod mouse;
mod offsets;
mod pmem;
mod process_protection;
mod util;
mod winver;

pub mod status;

extern crate alloc;

// FIXME: Exchange SyncUnsafeCell with a RwLock
pub static WSK: SyncUnsafeCell<Option<WskInstance>> = SyncUnsafeCell::new(None);
pub static REQUEST_HANDLER: SyncUnsafeCell<Option<Box<HandlerRegistry>>> =
    SyncUnsafeCell::new(Option::None);
pub static VALTHRUN_DEVICE: SyncUnsafeCell<Option<ValthrunDevice>> =
    SyncUnsafeCell::new(Option::None);
pub static KEYBOARD_INPUT: SyncUnsafeCell<Option<KeyboardInput>> =
    SyncUnsafeCell::new(Option::None);
pub static MOUSE_INPUT: SyncUnsafeCell<Option<MouseInput>> = SyncUnsafeCell::new(Option::None);
pub static METRICS_CLIENT: SyncUnsafeCell<Option<MetricsClient>> =
    SyncUnsafeCell::new(Option::None);
pub static METRICS_HEARTBEAT: SyncUnsafeCell<Option<MetricsHeartbeat>> = SyncUnsafeCell::new(None);

pub use logger::get_logger_instance;

/// Call this function to unload the driver.
/// Note: Also call when initialization failed.
pub extern "system" fn driver_unload() {
    log::info!("Unloading...");

    if let Some(metrics) = unsafe { &mut *METRICS_CLIENT.get() } {
        /* notify the metrics server about the unload */
        metrics.add_record(RECORD_TYPE_DRIVER_STATUS, "unload");
    }

    /* Remove the device */
    let device_handle = unsafe { &mut *VALTHRUN_DEVICE.get() };
    let _ = device_handle.take();

    /* Delete request handler registry */
    let request_handler = unsafe { &mut *REQUEST_HANDLER.get() };
    let _ = request_handler.take();

    /* Uninstall process protection */
    process_protection::finalize();

    let keyboard_input = unsafe { &mut *KEYBOARD_INPUT.get() };
    let _ = keyboard_input.take();

    let mouse_input = unsafe { &mut *MOUSE_INPUT.get() };
    let _ = mouse_input.take();

    let metrcis_heartbeat = unsafe { &mut *METRICS_HEARTBEAT.get() };
    if let Some(heartbeat) = metrcis_heartbeat.take() {
        heartbeat.shutdown();
    }

    let metrics = unsafe { &mut *METRICS_CLIENT.get() };
    if let Some(mut metrics) = metrics.take() {
        metrics.shutdown();
    }

    /* shutdown WSK after after everthing else has been shut down */
    let wsk = unsafe { &mut *WSK.get() };
    let _ = wsk.take();

    log::info!("Driver Unloaded");
}

pub fn internal_driver_entry(driver: &mut DRIVER_OBJECT) -> NTSTATUS {
    let mut error_code = STATUS_SUCCESS;

    match self::inner_driver_entry(driver, &mut error_code) {
        Ok(_) => STATUS_SUCCESS,
        Err(error) => {
            util::show_msgbox(
                obfstr!("Valthrun Kernel Driver"),
                &[
                    &format!(
                        "{} {}:",
                        obfstr!("Failed to initialize the Valthrun Kernel Driver version"),
                        env!("CARGO_PKG_VERSION")
                    ),
                    "",
                    &format!("Code: 0x{error_code:X}"),
                    &format!("Error: {error:?}"),
                    "",
                    obfstr!("For more information please refer to"),
                    obfstr!("https://wiki.valth.run/link/vtdk-1"),
                ]
                .join("\n"),
                MB_OK | MB_ICONERROR | MB_SYSTEMMODAL,
            );
            log::error!(
                "{} {error_code:X}: {error:#}",
                obfstr!("Initialisation failed with")
            );
            log::error!(
                "{}",
                obfstr!("For more information please refer to https://wiki.valth.run/link/vtdk-1")
            );
            error_code
        }
    }
}

pub fn inner_driver_entry(
    driver: &mut DRIVER_OBJECT,
    error_code: &mut NTSTATUS,
) -> anyhow::Result<()> {
    *error_code = CSTATUS_DRIVER_PREINIT_FAILED;
    initialize_os_info().with_context(|| obfstring!("initialize OS version information"))?;
    log::info!("WinVer {}", os_info().dwBuildNumber);

    *error_code = CSTATUS_DRIVER_INIT_FAILED;
    kapi::initialize(Some(driver)).with_context(|| obfstring!("initialize SEH"))?;

    unsafe {
        *WSK.get() =
            Some(WskInstance::create(1 << 8).with_context(|| obfstring!("initialize WSK"))?)
    };
    unsafe {
        *METRICS_CLIENT.get() =
            Some(metrics::initialize().with_context(|| obfstring!("initialize metrics"))?);

        *METRICS_HEARTBEAT.get() = Some(MetricsHeartbeat::new(Duration::from_secs(60 * 60)));
    };

    initialize_nt_offsets().with_context(|| obfstring!("initialize NT_OFFSETS"))?;
    unsafe {
        *KEYBOARD_INPUT.get() =
            Some(kb::create_keyboard_input().with_context(|| obfstring!("initialize keyboard"))?)
    };

    unsafe {
        *MOUSE_INPUT.get() =
            Some(mouse::create_mouse_input().with_context(|| obfstring!("initialize mouse"))?)
    };

    {
        /* extra error code for process protection */
        *error_code = CSTATUS_DRIVER_INIT_FAILED_PP;
        process_protection::initialize()
            .with_context(|| obfstring!("initialize process protection"))?;
        *error_code = CSTATUS_DRIVER_INIT_FAILED;
    }

    let device =
        ValthrunDevice::create(driver).with_context(|| obfstring!("create Valthrun device"))?;
    log::debug!(
        "{} device Object at 0x{:X} (Handle at 0x{:X})",
        obfstr!("Valthrun"),
        device.device_handle.device as *const _ as u64,
        &*device.device_handle as *const _ as u64
    );
    unsafe { *VALTHRUN_DEVICE.get() = Some(device) };

    let mut handler = Box::new(HandlerRegistry::new());

    handler.register(&handler_init);

    handler.register(&handler_read);
    handler.register(&handler_write);

    handler.register(&handler_get_modules);
    handler.register(&handler_get_processes);
    handler.register(&handler_protection_toggle);

    handler.register(&handler_mouse_move);
    handler.register(&handler_keyboard_state);

    handler.register(&handler_metrics_record);

    unsafe { *REQUEST_HANDLER.get() = Some(handler) };

    log::info!("Driver Initialized");
    Ok(())
}

pub fn metrics_client() -> Option<&'static MetricsClient> {
    let client = unsafe { &*METRICS_CLIENT.get() };
    client.as_ref()
}
