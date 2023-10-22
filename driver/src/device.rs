use alloc::{
    boxed::Box,
    format,
};
use core::pin::Pin;

use kapi::{
    DeviceHandle,
    IrpEx,
    Process,
    UnicodeStringEx,
};
use kdef::{
    IRP_MJ_CLOSE,
    IRP_MJ_CREATE,
    IRP_MJ_DEVICE_CONTROL,
    IRP_MJ_SHUTDOWN,
};
use obfstr::obfstr;
use winapi::{
    km::{
        ntifs::DEVICE_FLAGS,
        wdm::{
            IoGetCurrentIrpStackLocation,
            DEVICE_TYPE,
            DRIVER_OBJECT,
            IRP,
        },
    },
    shared::{
        guiddef::GUID,
        ntdef::{
            NTSTATUS,
            UNICODE_STRING,
        },
        ntstatus::{
            STATUS_INVALID_PARAMETER,
            STATUS_SUCCESS,
        },
    },
};

use crate::{
    imports::GLOBAL_IMPORTS,
    metrics::{
        RECORD_TYPE_DRIVER_IRP_STATUS,
        RECORD_TYPE_DRIVER_STATUS,
    },
    process_protection,
    METRICS_CLIENT,
    REQUEST_HANDLER,
};

type ValthrunDeviceHandle = DeviceHandle<()>;
pub struct ValthrunDevice {
    pub device_handle: Pin<Box<ValthrunDeviceHandle>>,
}

unsafe impl Sync for ValthrunDevice {}
impl ValthrunDevice {
    pub fn create(driver: &mut DRIVER_OBJECT) -> anyhow::Result<Self> {
        let imports = GLOBAL_IMPORTS.unwrap();

        let device_name = UNICODE_STRING::from_bytes(obfstr::wide!("\\Device\\valthrun"));
        let sddl =
            UNICODE_STRING::from_bytes(obfstr::wide!("D:P(A;;GA;;;SY)(A;;GA;;;BU)(A;;GA;;;AU)"));
        let mut guid = GUID::default();
        guid.Data1 = 0x3838266;
        guid.Data2 = 0x87FE;
        guid.Data3 = 0x4FEA;
        guid.Data4 = [0x1e, 0x79, 0xa8, 0xc2, 0xb8, 0x7c, 0x88, 0x0B];
        let mut device = DeviceHandle::<()>::create(
            driver,
            Some(&device_name),
            DEVICE_TYPE::FILE_DEVICE_UNKNOWN,
            0x00,
            false,
            &sddl,
            &guid,
            (),
        )?;

        device.major_function[IRP_MJ_CREATE] = Some(irp_create);
        device.major_function[IRP_MJ_CLOSE] = Some(irp_close);
        device.major_function[IRP_MJ_DEVICE_CONTROL] = Some(irp_control);
        device.major_function[IRP_MJ_SHUTDOWN] = Some(irp_shutdown);

        *device.flags_mut() |= DEVICE_FLAGS::DO_DIRECT_IO as u32;
        device.mark_initialized();

        unsafe { (imports.IoRegisterShutdownNotification)(device.device) };
        Ok(Self {
            device_handle: device,
        })
    }
}

impl Drop for ValthrunDevice {
    fn drop(&mut self) {
        let imports = GLOBAL_IMPORTS.unwrap();
        unsafe { (imports.IoUnregisterShutdownNotification)(self.device_handle.device) };
    }
}

fn irp_create(_device: &mut ValthrunDeviceHandle, irp: &mut IRP) -> NTSTATUS {
    log::trace!("{}", obfstr!("Valthrun IRP create callback"));

    let current_process = Process::current();
    if let Some(metrics) = unsafe { &*METRICS_CLIENT.get() } {
        metrics.add_record(
            RECORD_TYPE_DRIVER_IRP_STATUS,
            format!("open: {}", current_process.get_id()),
        );
    }

    irp.complete_request(STATUS_SUCCESS)
}

fn irp_close(_device: &mut ValthrunDeviceHandle, irp: &mut IRP) -> NTSTATUS {
    log::trace!("{}", obfstr!("Valthrun IRP close callback"));

    /*
     * Disable process protection for the process which is closing this driver.
     * A better solution would be to register a process termination callback
     * and remove the process ids from the protected list.
     */
    let current_process = Process::current();
    process_protection::toggle_protection(current_process.get_id(), false);

    if let Some(metrics) = unsafe { &*METRICS_CLIENT.get() } {
        metrics.add_record(
            RECORD_TYPE_DRIVER_IRP_STATUS,
            format!("close: {}", current_process.get_id()),
        );
    }

    irp.complete_request(STATUS_SUCCESS)
}

fn irp_control(_device: &mut ValthrunDeviceHandle, irp: &mut IRP) -> NTSTATUS {
    let outbuffer = irp.UserBuffer;
    let stack = unsafe { &mut *IoGetCurrentIrpStackLocation(irp) };
    let param = unsafe { stack.Parameters.DeviceIoControl() };
    let request_code = param.IoControlCode;

    let handler = match unsafe { REQUEST_HANDLER.get().as_ref() }
        .map(Option::as_ref)
        .flatten()
    {
        Some(handler) => handler,
        None => {
            log::warn!("Missing request handlers");
            return irp.complete_request(STATUS_INVALID_PARAMETER);
        }
    };

    /* Note: We do not lock the buffers as it's a sync call and the user should not be able to free the input buffers. */
    let inbuffer = unsafe {
        core::slice::from_raw_parts(
            param.Type3InputBuffer as *const u8,
            param.InputBufferLength as usize,
        )
    };

    if !seh::probe_read(inbuffer.as_ptr() as u64, inbuffer.len(), 1) {
        log::warn!("IRP request inbuffer invalid");
        return irp.complete_request(STATUS_INVALID_PARAMETER);
    }

    let outbuffer = unsafe {
        core::slice::from_raw_parts_mut(outbuffer as *mut u8, param.OutputBufferLength as usize)
    };
    if !seh::probe_write(outbuffer.as_ptr() as u64, outbuffer.len(), 1) {
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

fn irp_shutdown(_device: &mut ValthrunDeviceHandle, _irp: &mut IRP) -> NTSTATUS {
    log::debug!("{}", obfstr!("Received shutdown IRP"));

    if let Some(mut metrics) = unsafe { &mut *METRICS_CLIENT.get() }.take() {
        /* flush and shutdown metrics */
        metrics.add_record(RECORD_TYPE_DRIVER_STATUS, "shutdown");
        metrics.shutdown();
    }

    STATUS_SUCCESS
}
