#![no_std]
#![feature(core_intrinsics)]
#![feature(error_in_core)]
#![feature(sync_unsafe_cell)]
#![feature(pointer_byte_offsets)]
#![feature(result_flattening)]
#![feature(new_uninit)]
#![feature(const_transmute_copy)]
#![feature(linkage)]
#![feature(result_option_inspect)]
#![feature(naked_functions)]
#![allow(dead_code)]

use alloc::{
    boxed::Box,
    format,
};
use core::{
    cell::SyncUnsafeCell,
    time::Duration,
};

use device::ValthrunDevice;
use handler::HandlerRegistry;
use imports::LL_GLOBAL_IMPORTS;
use kapi::{
    device_general_irp_handler,
    NTStatusEx,
    UnicodeStringEx,
};
use kb::KeyboardInput;
use kdef::DPFLTR_LEVEL;
use metrics::{
    MetricsClient,
    MetricsHeartbeat,
};
use mouse::MouseInput;
use obfstr::obfstr;
use panic_hook::DEBUG_IMPORTS;
use valthrun_driver_shared::requests::RequestHealthCheck;
use winapi::{
    km::wdm::DRIVER_OBJECT,
    shared::{
        ntdef::{
            NTSTATUS,
            UNICODE_STRING,
        },
        ntstatus::{
            STATUS_OBJECT_NAME_COLLISION,
            STATUS_SUCCESS,
        },
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
    logger::APP_LOGGER,
    metrics::RECORD_TYPE_DRIVER_STATUS,
    offsets::initialize_nt_offsets,
    winver::{
        initialize_os_info,
        os_info,
    },
    wsk::WskInstance,
};

extern crate compiler_builtins;

mod device;
mod handler;
mod imports;
mod io;
mod kb;
mod logger;
mod metrics;
mod mouse;
mod offsets;
mod panic_hook;
mod pmem;
mod process_protection;
mod util;
mod winver;
mod wsk;

mod status;
use status::*;

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

extern "system" fn driver_unload(_driver: &mut DRIVER_OBJECT) {
    real_driver_unload();
}

fn real_driver_unload() {
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

#[no_mangle]
pub extern "system" fn driver_entry(
    driver: *mut DRIVER_OBJECT,
    registry_path: *const UNICODE_STRING,
) -> NTSTATUS {
    utils_imports::initialize();
    if DEBUG_IMPORTS.resolve().is_err() {
        /*
         * If this import fails, we can't do anything else except return an appropiate status code.
         */
        return CSTATUS_DRIVER_BOOTSTRAP_FAILED;
    }

    log::set_max_level(log::LevelFilter::Trace);
    if log::set_logger(&APP_LOGGER).is_err() {
        let imports = DEBUG_IMPORTS.unwrap();
        unsafe {
            (imports.DbgPrintEx)(
                0,
                DPFLTR_LEVEL::ERROR as u32,
                obfstr!("[VT] Failed to initialize app logger!\n\0").as_ptr(),
            );
        }

        return CSTATUS_LOG_INIT_FAILED;
    }

    let ll_imports = match LL_GLOBAL_IMPORTS.resolve() {
        Ok(imports) => imports,
        Err(error) => {
            log::error!(
                "{}: {:#}",
                obfstr!("Failed to initialize ll imports"),
                error
            );
            return CSTATUS_DRIVER_PREINIT_FAILED;
        }
    };

    if let Err(error) = initialize_os_info() {
        log::error!("{}: {}", obfstr!("Failed to load OS version info"), error);
        return CSTATUS_DRIVER_PREINIT_FAILED;
    }

    let status = match unsafe { driver.as_mut() } {
        Some(driver) => internal_driver_entry(driver, registry_path),
        None => {
            log::info!("{}", obfstr!("Manually mapped driver."));
            log::debug!(
                "  {} {:X}.",
                obfstr!("System range start is"),
                ll_imports.MmSystemRangeStart as u64,
            );
            log::debug!("  {} {:X}", obfstr!("IRQL level at"), unsafe {
                (ll_imports.KeGetCurrentIrql)()
            });

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

            // To unload (Unload is not called):
            // if(gDriverObject->DriverUnload) {
            // gDriverObject->DriverUnload(gDriverObject);
            // }

            // ObMakeTemporaryObject (gDriverObject);
            // IoDeleteDriver (gDriverObject);
            // gDriverObject = NULL;
        }
    };

    if let Some(metrics) = unsafe { &*METRICS_CLIENT.get() } {
        /* report the load result if metrics could be already initialized */
        metrics.add_record(
            RECORD_TYPE_DRIVER_STATUS,
            format!(
                "load:{:X}, version:{}, manual:{:X}",
                status,
                env!("CARGO_PKG_VERSION"),
                driver.is_null() as u8
            ),
        );
    }

    if status != STATUS_SUCCESS {
        /* cleanup all pending / initialized resources */
        real_driver_unload();
    }

    status
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
            "Initialize driver at {:X} ({:?}). WinVer {}. Kernel base at {:X}",
            driver as *mut _ as u64,
            registry_path,
            os_info().dwBuildNumber,
            // unsafe { *KERNEL_BASE.get() } // FIXME: Reimplement
            0
        );
    }

    driver.DriverUnload = Some(driver_unload);
    if let Err(error) = GLOBAL_IMPORTS.resolve() {
        log::error!(
            "{}: {:#}",
            obfstr!("Failed to load the global import table"),
            error
        );
        return CSTATUS_DRIVER_INIT_FAILED;
    }

    if let Err(error) = seh::init() {
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

    for function in driver.MajorFunction.iter_mut() {
        *function = Some(device_general_irp_handler);
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
