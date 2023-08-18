use core::pin::Pin;

use alloc::boxed::Box;
use winapi::{km::{wdm::{DEVICE_TYPE, DRIVER_OBJECT, IoCreateSymbolicLink, IoDeleteSymbolicLink, IRP, IoGetCurrentIrpStackLocation}, ntifs::DEVICE_FLAGS}, shared::{ntdef::{UNICODE_STRING, NTSTATUS}, ntstatus::{STATUS_SUCCESS, STATUS_INVALID_PARAMETER}}};

use crate::{kapi::{DeviceHandle, UnicodeStringEx, NTStatusEx, IrpEx, Process, self, mem}, process_protection, REQUEST_HANDLER, kdef::{ProbeForRead, ProbeForWrite, IRP_MJ_CREATE, IRP_MJ_CLOSE, IRP_MJ_DEVICE_CONTROL}};

type VarhalDeviceHandle = DeviceHandle<()>;
pub struct VarhalDevice {
    pub device_handle: Pin<Box<VarhalDeviceHandle>>,
    dos_link_name: UNICODE_STRING,
}

unsafe impl Sync for VarhalDevice {}
impl VarhalDevice {
    pub fn create(driver: &mut DRIVER_OBJECT) -> anyhow::Result<Self> {
        let dos_name = UNICODE_STRING::from_bytes(obfstr::wide!("\\DosDevices\\valthrun"));
        let device_name = UNICODE_STRING::from_bytes(obfstr::wide!("\\Device\\valthrun"));

        let mut device = DeviceHandle::<()>::create(
            driver,  
            Some(&device_name), 
            DEVICE_TYPE::FILE_DEVICE_UNKNOWN, // FILE_DEVICE_UNKNOWN
            0x00000100, // FILE_DEVICE_SECURE_OPEN
            false, 
            ()
        )?;

        device.major_function[IRP_MJ_CREATE] = Some(irp_create);
        device.major_function[IRP_MJ_CLOSE] = Some(irp_close);
        device.major_function[IRP_MJ_DEVICE_CONTROL] = Some(irp_control);
    
        unsafe {
            IoCreateSymbolicLink(&dos_name, &device_name)
                .ok()
                .map_err(|err| anyhow::anyhow!("IoCreateSymbolicLink: {}", err))?;
        };
    
        *device.flags_mut() |= DEVICE_FLAGS::DO_DIRECT_IO as u32;
        device.mark_initialized();
        Ok(Self {
            device_handle: device,
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


fn irp_create(_device: &mut VarhalDeviceHandle, irp: &mut IRP) -> NTSTATUS {
    log::debug!("Varhal IRP create callback");

    irp.complete_request(STATUS_SUCCESS)
}

fn irp_close(_device: &mut VarhalDeviceHandle, irp: &mut IRP) -> NTSTATUS {
    log::debug!("Varhal IRP close callback");

    /*
     * Disable process protection for the process which is closing this driver.
     * A better solution would be to register a process termination callback
     * and remove the process ids from the protected list.
     */
    let current_process = Process::current();
    process_protection::toggle_protection(current_process.get_id(), false);

    irp.complete_request(STATUS_SUCCESS)
}

fn irp_control(_device: &mut VarhalDeviceHandle, irp: &mut IRP) -> NTSTATUS {
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
    
    if !mem::probe_read(inbuffer.as_ptr() as u64, inbuffer.len(), 1) {
        log::warn!("IRP request inbuffer invalid");
        return irp.complete_request(STATUS_INVALID_PARAMETER);
    }

    let outbuffer = unsafe {
        core::slice::from_raw_parts_mut(outbuffer as *mut u8, param.OutputBufferLength as usize)
    };
    if !mem::probe_write(outbuffer.as_ptr() as u64, outbuffer.len(), 1) {
        log::warn!("IRP request outbuffer invalid");
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
