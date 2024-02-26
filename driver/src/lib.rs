#![no_std]
#![feature(error_in_core)]
#![feature(sync_unsafe_cell)]
#![feature(result_flattening)]
#![feature(new_uninit)]
#![feature(linkage)]
#![feature(naked_functions)]
#![feature(core_intrinsics)]
#![allow(internal_features)]
#![allow(dead_code)]

use alloc::boxed::Box;
use core::{
    cell::SyncUnsafeCell,
    time::Duration,
};

use device::ValthrunDevice;
use handler::HandlerRegistry;
use kb::KeyboardInput;
use metrics::{
    MetricsClient,
    MetricsHeartbeat,
};
use mouse::MouseInput;
use obfstr::obfstr;
use valthrun_driver_shared::requests::RequestHealthCheck;
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
        handler_get_modules,
        handler_init,
        handler_keyboard_state,
        handler_metrics_record,
        handler_mouse_move,
        handler_protection_toggle,
        handler_read,
        handler_write,
    },
    imports::GLOBAL_IMPORTS,
    metrics::RECORD_TYPE_DRIVER_STATUS,
    offsets::initialize_nt_offsets,
    status::{
        CSTATUS_DRIVER_INIT_FAILED,
        CSTATUS_DRIVER_PREINIT_FAILED,
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

fn wsk_dummy() -> anyhow::Result<()> {
    // if let Some(metrics) = unsafe { &*METRICS_CLIENT.get() } {
    //     for i in 0..1_000 {
    //         metrics.add_record(format!("testing_{}", i), "some payload but the content is a little longer so it will trigger the message too long issue");
    //     }
    // }

    // let wsk = unsafe { &*WSK.get() };
    // let wsk = wsk.as_ref().context("missing WSK instance")?;
    // match metrics::send_report(&wsk, "/report", "{ \"message\": \"Hello World?\" }") {
    //     Ok(_) => {
    //         log::debug!("Success!");
    //     }
    //     Err(error) => {
    //         log::debug!("Fail: {:#}", error);
    //     }
    // }

    Ok(())
}

pub fn internal_driver_entry(driver: &mut DRIVER_OBJECT) -> NTSTATUS {
    if let Err(error) = initialize_os_info() {
        log::error!("{}: {}", obfstr!("Failed to load OS version info"), error);
        return CSTATUS_DRIVER_PREINIT_FAILED;
    }

    log::info!("WinVer {}", os_info().dwBuildNumber);
    if let Err(error) = GLOBAL_IMPORTS.resolve() {
        log::error!(
            "{}: {:#}",
            obfstr!("Failed to load the global import table"),
            error
        );
        return CSTATUS_DRIVER_INIT_FAILED;
    }

    if let Err(error) = kapi::initialize(Some(driver)) {
        log::error!("{}: {:#}", obfstr!("Failed to initialize SEH"), error);
        return CSTATUS_DRIVER_INIT_FAILED;
    }

    match WskInstance::create(1 << 8) {
        Ok(wsk) => {
            unsafe { *WSK.get() = Some(wsk) };
        }
        Err(err) => {
            log::error!("{}: {:#}", obfstr!("WSK initialize error"), err);
            return CSTATUS_DRIVER_INIT_FAILED;
        }
    }

    match metrics::initialize() {
        Err(error) => {
            log::error!("{}: {:#}", obfstr!("Failed to initialize metrics"), error);
            return CSTATUS_DRIVER_INIT_FAILED;
        }
        Ok(client) => {
            unsafe { *METRICS_CLIENT.get() = Some(client) };
        }
    }

    unsafe { *METRICS_HEARTBEAT.get() = Some(MetricsHeartbeat::new(Duration::from_secs(60 * 60))) };

    if let Err(err) = wsk_dummy() {
        log::error!("{}: {:#}", obfstr!("WSK dummy error"), err);
        return CSTATUS_DRIVER_INIT_FAILED;
    }

    if let Err(error) = initialize_nt_offsets() {
        log::error!(
            "{}: {:#}",
            obfstr!("Failed to initialize NT_OFFSETS"),
            error
        );
        return CSTATUS_DRIVER_INIT_FAILED;
    }

    match kb::create_keyboard_input() {
        Err(error) => {
            log::error!(
                "{} {:#}",
                obfstr!("Failed to initialize keyboard input:"),
                error
            );
            return CSTATUS_DRIVER_INIT_FAILED;
        }
        Ok(keyboard) => {
            unsafe { *KEYBOARD_INPUT.get() = Some(keyboard) };
        }
    }

    match mouse::create_mouse_input() {
        Err(error) => {
            log::error!(
                "{} {:#}",
                obfstr!("Failed to initialize mouse input:"),
                error
            );
            return CSTATUS_DRIVER_INIT_FAILED;
        }
        Ok(mouse) => {
            unsafe { *MOUSE_INPUT.get() = Some(mouse) };
        }
    }

    if let Err(error) = process_protection::initialize() {
        log::error!(
            "{} {:#}",
            obfstr!("Failed to initialized process protection:"),
            error
        );
        return CSTATUS_DRIVER_INIT_FAILED;
    };

    let device = match ValthrunDevice::create(driver) {
        Ok(device) => device,
        Err(error) => {
            log::error!("{} {:#}", obfstr!("Failed to initialize device:"), error);
            return CSTATUS_DRIVER_INIT_FAILED;
        }
    };
    log::debug!(
        "{} device Object at 0x{:X} (Handle at 0x{:X})",
        obfstr!("Valthrun"),
        device.device_handle.device as *const _ as u64,
        &*device.device_handle as *const _ as u64
    );
    unsafe { *VALTHRUN_DEVICE.get() = Some(device) };

    let mut handler = Box::new(HandlerRegistry::new());

    handler.register(&|_req: &RequestHealthCheck, res| {
        res.success = true;
        Ok(())
    });
    handler.register(&handler_get_modules);
    handler.register(&handler_read);
    handler.register(&handler_write);
    handler.register(&handler_protection_toggle);
    handler.register(&handler_mouse_move);
    handler.register(&handler_keyboard_state);
    handler.register(&handler_init);
    handler.register(&handler_metrics_record);

    unsafe { *REQUEST_HANDLER.get() = Some(handler) };

    log::info!("Driver Initialized");
    STATUS_SUCCESS
}

pub fn metrics_client() -> Option<&'static MetricsClient> {
    let client = unsafe { &*METRICS_CLIENT.get() };
    client.as_ref()
}
