#![no_std]
#![feature(sync_unsafe_cell)]
#![feature(pointer_byte_offsets)]
#![feature(result_flattening)]

use core::{cell::SyncUnsafeCell, mem::size_of_val, ffi::CStr};

use alloc::string::{String, ToString};
use handler::HandlerRegistry;
use kapi::{DeviceHandle, UnicodeStringEx, NTStatusEx};
use kdef::{ProbeForRead, ProbeForWrite};
use valthrun_driver_shared::requests::{RequestHealthCheck, RequestCSModule, RequestRead, RequestProtectionToggle};
use winapi::{shared::{ntdef::{UNICODE_STRING, NTSTATUS}, ntstatus::{STATUS_SUCCESS, STATUS_INVALID_PARAMETER, STATUS_FAILED_DRIVER_ENTRY, STATUS_OBJECT_NAME_COLLISION}}, km::wdm::{DRIVER_OBJECT, DEVICE_TYPE, DEVICE_FLAGS, IoCreateSymbolicLink, IoDeleteSymbolicLink, DEVICE_OBJECT, IRP, IoGetCurrentIrpStackLocation, PEPROCESS, DbgPrintEx}};

use crate::{logger::APP_LOGGER, handler::{handler_get_modules, handler_read, handler_protection_toggle}, kdef::{DPFLTR_LEVEL, MmSystemRangeStart, IoCreateDriver, KeGetCurrentIrql}, kapi::{IrpEx, Process}};

mod panic_hook;
mod logger;
mod handler;
mod kapi;
mod kdef;
mod process_protection;

extern crate alloc;

static REQUEST_HANDLER: SyncUnsafeCell<Option<HandlerRegistry>> = SyncUnsafeCell::new(Option::None);
static VARHAL_DEVICE: SyncUnsafeCell<Option<VarhalDevice>> = SyncUnsafeCell::new(Option::None);

struct VarhalDevice {
    _device: DeviceHandle,
    dos_link_name: UNICODE_STRING,
}

unsafe impl Sync for VarhalDevice {}
impl VarhalDevice {
    pub fn create(driver: &mut DRIVER_OBJECT) -> anyhow::Result<Self> {
        let dos_name = UNICODE_STRING::from_bytes(obfstr::wide!("\\DosDevices\\valthrun"));
        let device_name = UNICODE_STRING::from_bytes(obfstr::wide!("\\Device\\valthrun"));

        let mut device = DeviceHandle::create(
            driver,  
            &device_name, 
            DEVICE_TYPE::FILE_DEVICE_UNKNOWN, // FILE_DEVICE_UNKNOWN
            0x00000100, // FILE_DEVICE_SECURE_OPEN
            false, 
        )?;
    
        unsafe {
            IoCreateSymbolicLink(&dos_name, &device_name)
                .ok()
                .map_err(|err| anyhow::anyhow!("IoCreateSymbolicLink: {}", err))?;
        };
    
        *device.flags_mut() |= DEVICE_FLAGS::DO_DIRECT_IO as u32;
        device.mark_initialized();
        Ok(Self {
            _device: device,
            dos_link_name: dos_name
        })
    }
}

impl Drop for VarhalDevice {
    fn drop(&mut self) {
        let result = unsafe { IoDeleteSymbolicLink(&self.dos_link_name) };
        if let Err(status) = result.ok() {
            log::warn!("Failed to unlink dos device: {}", status);
        }
    }
}

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

    log::info!("Driver Unloaded");
}

extern "system" fn irp_create(_device: &mut DEVICE_OBJECT, irp: &mut IRP) -> NTSTATUS {
    log::debug!("IRP create callback");

    irp.complete_request(STATUS_SUCCESS)
}

extern "system" fn irp_close(_device: &mut DEVICE_OBJECT, irp: &mut IRP) -> NTSTATUS {
    log::debug!("IRP close callback");

    /*
     * Disable process protection for the process which is closing this driver.
     * A better solution would be to register a process termination callback
     * and remove the process ids from the protected list.
     */
    let current_process = Process::current();
    process_protection::toggle_protection(current_process.get_id(), false);

    irp.complete_request(STATUS_SUCCESS)
}

extern "system" fn irp_control(_device: &mut DEVICE_OBJECT, irp: &mut IRP) -> NTSTATUS {
    let outbuffer = irp.UserBuffer;
    let stack = unsafe { &mut *IoGetCurrentIrpStackLocation(irp) };
    let param = unsafe { stack.Parameters.DeviceIoControl() };
    let request_code = param.IoControlCode;

    let handler = match unsafe { REQUEST_HANDLER.get().as_ref() }.map(Option::as_ref).flatten() {
        Some(handler) => handler,
        None => {
            log::warn!("Missing request handlers");
            return irp.complete_request(STATUS_INVALID_PARAMETER);
        }
    };

    /* Note: We do not lock the buffers as it's a sync call and the user should not be able to free the input buffers. */
    let inbuffer = unsafe {
        core::slice::from_raw_parts(param.Type3InputBuffer as *const u8, param.InputBufferLength as usize)
    };
    let inbuffer_probe = kapi::try_seh(|| unsafe {
        ProbeForRead(inbuffer.as_ptr() as *const (), inbuffer.len(), 1);
    });
    if let Err(err) = inbuffer_probe {
        log::warn!("IRP request inbuffer invalid: {}", err);
        return irp.complete_request(STATUS_INVALID_PARAMETER);
    }

    let outbuffer = unsafe {
        core::slice::from_raw_parts_mut(outbuffer as *mut u8, param.OutputBufferLength as usize)
    };
    let outbuffer_probe = kapi::try_seh(|| unsafe {
        ProbeForWrite(outbuffer.as_mut_ptr() as *mut (), outbuffer.len(), 1);
    });
    if let Err(err) = outbuffer_probe {
        log::warn!("IRP request outbuffer invalid: {}", err);
        return irp.complete_request(STATUS_INVALID_PARAMETER);
    }

    match handler.handle(request_code, inbuffer, outbuffer) {
        Ok(_) => irp.complete_request(STATUS_SUCCESS),
        Err(error) => {
            log::error!("IRP handle error: {}", error);
            irp.complete_request(STATUS_INVALID_PARAMETER)
        }
    }
}

#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
struct _OSVERSIONINFOEXW {
    dwOSVersionInfoSize: u32,
    dwMajorVersion: u32,
    dwMinorVersion: u32,
    dwBuildNumber: u32,
    dwPlatformId: u32,

    szCSDVersion: [u16; 128],
    wServicePackMajor: u16,
    wServicePackMinor: u16,
    wSuiteMask: u16,

    wProductType: u8,
    wReserved: u8
}

extern "system" {
    fn RtlGetVersion(info: &mut _OSVERSIONINFOEXW) -> NTSTATUS;
}

pub fn get_windows_build_number() -> anyhow::Result<u32, NTSTATUS> {
    let mut info: _OSVERSIONINFOEXW = unsafe { core::mem::zeroed() };
    info.dwOSVersionInfoSize = size_of_val(&info) as u32;
    unsafe { RtlGetVersion(&mut info) }
        .ok()
        .map(|_| info.dwBuildNumber)
}

// TODO: Move into the process itself?
fn get_process_name<'a>(handle: PEPROCESS) -> Option<String> {
    let image_file_name = unsafe {
        (handle as *const ()).byte_offset(0x5a8) // FIXME: Hardcoded offset ImageFileName
            .cast::<[u8; 15]>()
            .read()
    };

    CStr::from_bytes_until_nul(image_file_name.as_slice())
        .map(|value| value.to_str().ok())
        .ok()
        .flatten()
        .map(|s| s.to_string())
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

extern "C" fn internal_driver_entry(driver: &mut DRIVER_OBJECT, _registry_path: *const UNICODE_STRING) -> NTSTATUS {
    log::info!("Initialize driver at {:X}.", driver as *mut _ as u64);

    driver.DriverUnload = Some(driver_unload);
    driver.MajorFunction[0x00] = Some(irp_create); /* IRP_MJ_CREATE */
    driver.MajorFunction[0x02] = Some(irp_close); /* IRP_MJ_CLOSE */
    driver.MajorFunction[0x0E] = Some(irp_control); /* IRP_MJ_DEVICE_CONTROL */

    
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
    log::debug!("Driver Object at 0x{:X}, Device Object at 0x{:X}", driver as *const _ as u64, device._device.0 as *const _ as u64);
    unsafe { *VARHAL_DEVICE.get() = Some(device) };

    let mut handler = HandlerRegistry::new();
    handler.register::<RequestHealthCheck>(&|_req, res| {
        res.success = true;
        Ok(())
    });
    handler.register::<RequestCSModule>(&handler_get_modules);
    handler.register::<RequestRead>(&handler_read);
    handler.register::<RequestProtectionToggle>(&handler_protection_toggle);

    unsafe { *REQUEST_HANDLER.get() = Some(handler) };

    log::info!("Driver Initialized");
    STATUS_SUCCESS
}
