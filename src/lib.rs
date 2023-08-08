#![no_std]
#![feature(sync_unsafe_cell)]
#![feature(pointer_byte_offsets)]
#![feature(result_flattening)]
#![feature(new_uninit)]
#![feature(const_transmute_copy)]

use core::cell::SyncUnsafeCell;

use alloc::boxed::Box;
use device_varhal::VarhalDevice;
use handler::HandlerRegistry;
use kapi::{UnicodeStringEx, NTStatusEx};
use kb::KeyboardInput;
use mouse::MouseInput;
use obfstr::obfstr;
use valthrun_driver_shared::requests::{RequestHealthCheck, RequestCSModule, RequestRead, RequestProtectionToggle, RequestMouseMove, RequestKeyboardState};
use winapi::{shared::{ntdef::{UNICODE_STRING, NTSTATUS}, ntstatus::{STATUS_SUCCESS, STATUS_FAILED_DRIVER_ENTRY, STATUS_OBJECT_NAME_COLLISION}}, km::wdm::{DRIVER_OBJECT, DbgPrintEx}};

use crate::{logger::APP_LOGGER, handler::{handler_get_modules, handler_read, handler_protection_toggle, handler_mouse_move, handler_keyboard_state}, kdef::{DPFLTR_LEVEL, MmSystemRangeStart, IoCreateDriver, KeGetCurrentIrql}, kapi::device_general_irp_handler, offsets::initialize_nt_offsets, winver::{initialize_os_info, OS_VERSION_INFO}};

mod panic_hook;
mod logger;
mod handler;
mod kapi;
mod kdef;
mod offsets;
mod process_protection;
mod winver;
mod device_varhal;
mod kb;
mod mouse;

extern crate alloc;

static REQUEST_HANDLER: SyncUnsafeCell<Option<Box<HandlerRegistry>>> = SyncUnsafeCell::new(Option::None);
static VARHAL_DEVICE: SyncUnsafeCell<Option<VarhalDevice>> = SyncUnsafeCell::new(Option::None);
static KEYBOARD_INPUT: SyncUnsafeCell<Option<KeyboardInput>> = SyncUnsafeCell::new(Option::None);
static MOUSE_INPUT: SyncUnsafeCell<Option<MouseInput>> = SyncUnsafeCell::new(Option::None);

#[no_mangle]
extern "system" fn driver_unload(_driver: &mut DRIVER_OBJECT) {
    log::info!("Unloading...");

    /* Remove the device */
    let device_handle = unsafe { &mut *VARHAL_DEVICE.get() };
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

    log::info!("Driver Unloaded");
}

#[no_mangle]
pub extern "system" fn driver_entry(driver: *mut DRIVER_OBJECT, registry_path: *const UNICODE_STRING) -> NTSTATUS {
    log::set_max_level(log::LevelFilter::Trace);
    if log::set_logger(&APP_LOGGER).is_err() {
        unsafe { 
            DbgPrintEx(0, DPFLTR_LEVEL::ERROR as u32, "[VT] Failed to initialize app logger!\n\0".as_ptr());
        }

        return STATUS_FAILED_DRIVER_ENTRY;
    }

    if let Err(error) = initialize_os_info() {
        log::error!("Failed to load OS version info: {:X}", error);
        return STATUS_FAILED_DRIVER_ENTRY;
    }

    log::info!("Driver entry called");
    match unsafe { driver.as_mut() } {
        Some(driver) => internal_driver_entry(driver, registry_path),
        None => {
            let target_driver_entry = internal_driver_entry as usize;
            log::info!("Manually mapped drive.");
            log::info!("  System range start is {:X}, driver entry mapped at {:X}.", unsafe { MmSystemRangeStart } as u64, target_driver_entry);
            log::info!("  IRQL level at {:X}", unsafe { KeGetCurrentIrql() });

            // TODO(low): May improve hiding via:
            // https://research.checkpoint.com/2021/a-deep-dive-into-doublefeature-equation-groups-post-exploitation-dashboard/
            let driver_name = UNICODE_STRING::from_bytes(obfstr::wide!("\\Driver\\valthrun-driver"));
            let result = unsafe { IoCreateDriver(&driver_name, target_driver_entry as *const _) };
            if let Err(code) = result.ok() {
                if code == STATUS_OBJECT_NAME_COLLISION {
                    log::error!("Failed to create valthrun driver as a driver with this name is already loaded.");
                } else {
                    log::error!("Failed to create new driver for manually mapped driver: {:X}", code);
                }
                STATUS_FAILED_DRIVER_ENTRY
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
    }
}

extern "C" fn internal_driver_entry(driver: &mut DRIVER_OBJECT, registry_path: *const UNICODE_STRING) -> NTSTATUS {
    let registry_path = unsafe { registry_path.as_ref() }.map(|path| path.as_string_lossy());
    {
        let registry_path = registry_path.as_ref()
            .map(|path| path.as_str())
            .unwrap_or("None");

        log::info!("Initialize driver at {:X} ({:?}). WinVer {}.", driver as *mut _ as u64, registry_path, OS_VERSION_INFO.dwBuildNumber);
    }

    /* Needs to be done first as it's assumed to be init */
    if let Err(error) = initialize_nt_offsets() {
        log::error!("{}: {}", obfstr!("Failed to initialize NT_OFFSETS: {:#}"), error);
        return STATUS_FAILED_DRIVER_ENTRY;
    }

    driver.DriverUnload = Some(driver_unload);
    for function in driver.MajorFunction.iter_mut() {
        *function = Some(device_general_irp_handler);
    }

    match kb::create_keyboard_input() {
        Err(error) => {
            log::error!("Failed to initialize keyboard input: {:#}", error);
            return STATUS_FAILED_DRIVER_ENTRY;
        },
        Ok(keyboard) => {
            unsafe { *KEYBOARD_INPUT.get() = Some(keyboard) };
        }
    }

    match mouse::create_mouse_input() {
        Err(error) => {
            log::error!("Failed to initialize mouse input: {:#}", error);
            return STATUS_FAILED_DRIVER_ENTRY;
        },
        Ok(mouse) => {
            unsafe { *MOUSE_INPUT.get() = Some(mouse) };
        }
    }
    
    if let Err(error) = process_protection::initialize() {
        log::error!("Failed to initialized process protection: {:#}", error);
        return STATUS_FAILED_DRIVER_ENTRY;
    };
    
    let device = match VarhalDevice::create(driver) {
        Ok(device) => device,
        Err(error) => {
            log::error!("Failed to initialize device: {:#}", error);
            return STATUS_FAILED_DRIVER_ENTRY;
        }
    };
    log::debug!("Varhal device Object at 0x{:X} (Handle at 0x{:X})", 
        device.device_handle.device as *const _ as u64, &*device.device_handle as *const _ as u64);
    unsafe { *VARHAL_DEVICE.get() = Some(device) };

    let mut handler = Box::new(HandlerRegistry::new());
    
    handler.register::<RequestHealthCheck>(&|_req, res| {
        res.success = true;
        Ok(())
    });
    handler.register::<RequestCSModule>(&handler_get_modules);
    handler.register::<RequestRead>(&handler_read);
    handler.register::<RequestProtectionToggle>(&handler_protection_toggle);
    handler.register::<RequestMouseMove>(&handler_mouse_move);
    handler.register::<RequestKeyboardState>(&handler_keyboard_state);

    unsafe { *REQUEST_HANDLER.get() = Some(handler) };

    log::info!("Driver Initialized");
    STATUS_SUCCESS
}
