use core::pin::Pin;

use alloc::boxed::Box;
use winapi::{km::wdm::{PDEVICE_OBJECT, DRIVER_OBJECT, IoCreateDevice, DEVICE_TYPE, DEVICE_FLAGS, IoDeleteDevice, DEVICE_OBJECT, IRP, IoGetCurrentIrpStackLocation}, shared::{ntdef::{UNICODE_STRING, NTSTATUS}, ntstatus::STATUS_NOT_SUPPORTED}};

use crate::kapi::IrpEx;

use super::NTStatusEx;

type DeviceMajorFn<T> = fn(device: &mut DeviceHandle<T>, irp: &mut IRP) -> NTSTATUS;

pub struct DeviceHandle<T> {
    pub device: PDEVICE_OBJECT,
    pub major_function: [Option<DeviceMajorFn<T>>; 28],
    pub data: T,
}
unsafe impl<T: Sync> Sync for DeviceHandle<T> {}

impl<T> DeviceHandle<T> {
    pub fn create(
        driver: &mut DRIVER_OBJECT,
        device_name: Option<&UNICODE_STRING>,
        device_type: DEVICE_TYPE,
        characteristics: u32,
        exclusive: bool,
        data: T,
    ) -> anyhow::Result<Pin<Box<Self>>> {
        let mut device_ptr: PDEVICE_OBJECT = core::ptr::null_mut();
        let result = unsafe {
            IoCreateDevice(
                driver, core::mem::size_of::<*const ()>() as u32, 
                device_name.map(|name| name as *const _).unwrap_or(core::ptr::null()), 
                device_type,
                characteristics,
                if exclusive { 1 } else { 0 }, 
                &mut device_ptr
            )
        };

        if !result.is_ok() {
            anyhow::bail!("IoCreateDevice failed with {}", result)
        }

        let result = Box::pin(Self {
            device: device_ptr,
            major_function: Default::default(),
            data
        });

        unsafe {
            (*device_ptr).DeviceExtension = &*result as *const _ as *mut _;
        }
        Ok(result)
    }
    
    pub fn flags(&self) -> u32 {
        unsafe { (*self.device).Flags }
    }

    pub fn flags_mut(&mut self) -> &mut u32 {
        unsafe { &mut (*self.device).Flags }
    }

    pub fn mark_initialized(&mut self) {
        unsafe {
            (*self.device).Flags &= !(DEVICE_FLAGS::DO_DEVICE_INITIALIZING as u32);
        }
    }
}

impl<T> Drop for DeviceHandle<T> {
    fn drop(&mut self) {
        let result = unsafe { IoDeleteDevice(&mut *self.device) };

        if !result.is_success() {
            log::warn!("Failed to destroy device: {}", result)
        }
    }
}

pub extern "system" fn device_general_irp_handler(device: &mut DEVICE_OBJECT, irp: &mut IRP) -> NTSTATUS {
    let device_handle = unsafe { (device.DeviceExtension as *mut DeviceHandle<()>).as_mut() };
    let device_handle = match device_handle {
        Some(handle) => handle,
        None => {
            log::error!("General IRP handler called without a valid device handle.");
            return irp.complete_request(STATUS_NOT_SUPPORTED);
        }
    };

    let stack = unsafe { &mut *IoGetCurrentIrpStackLocation(irp) };
    let major_function_index = stack.MajorFunction as usize;
    debug_assert!(major_function_index < device_handle.major_function.len());
    if let Some(handler) = &device_handle.major_function[major_function_index] {
        // log::trace!("IRP 0x{:0>2X} dispatch {:X}", major_function_index, device_handle.device as u64);
        handler(device_handle, irp)
    } else {
        // log::trace!("IRP 0x{:0>2X} not supported on {:X}", major_function_index, device_handle.device as u64);
        irp.complete_request(STATUS_NOT_SUPPORTED)
    }
}