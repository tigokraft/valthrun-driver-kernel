#![allow(unused)]

use winapi::{
    km::wdm::{
        IoGetCurrentIrpStackLocation,
        IRP,
        PIO_COMPLETION_ROUTINE,
        PIO_STACK_LOCATION,
        PIRP,
        SL_INVOKE_ON_CANCEL,
        SL_INVOKE_ON_ERROR,
        SL_INVOKE_ON_SUCCESS,
    },
    shared::ntdef::PVOID,
};

pub const IRP_MJ_CREATE: usize = 0x00;
pub const IRP_MJ_CREATE_NAMED_PIPE: usize = 0x01;
pub const IRP_MJ_CLOSE: usize = 0x02;
pub const IRP_MJ_READ: usize = 0x03;
pub const IRP_MJ_WRITE: usize = 0x04;
pub const IRP_MJ_QUERY_INFORMATION: usize = 0x05;
pub const IRP_MJ_SET_INFORMATION: usize = 0x06;
pub const IRP_MJ_QUERY_EA: usize = 0x07;
pub const IRP_MJ_SET_EA: usize = 0x08;
pub const IRP_MJ_FLUSH_BUFFERS: usize = 0x09;
pub const IRP_MJ_QUERY_VOLUME_INFORMATION: usize = 0x0a;
pub const IRP_MJ_SET_VOLUME_INFORMATION: usize = 0x0b;
pub const IRP_MJ_DIRECTORY_CONTROL: usize = 0x0c;
pub const IRP_MJ_FILE_SYSTEM_CONTROL: usize = 0x0d;
pub const IRP_MJ_DEVICE_CONTROL: usize = 0x0e;
pub const IRP_MJ_INTERNAL_DEVICE_CONTROL: usize = 0x0f;
pub const IRP_MJ_SHUTDOWN: usize = 0x10;
pub const IRP_MJ_LOCK_CONTROL: usize = 0x11;
pub const IRP_MJ_CLEANUP: usize = 0x12;
pub const IRP_MJ_CREATE_MAILSLOT: usize = 0x13;
pub const IRP_MJ_QUERY_SECURITY: usize = 0x14;
pub const IRP_MJ_SET_SECURITY: usize = 0x15;
pub const IRP_MJ_POWER: usize = 0x16;
pub const IRP_MJ_SYSTEM_CONTROL: usize = 0x17;
pub const IRP_MJ_DEVICE_CHANGE: usize = 0x18;
pub const IRP_MJ_QUERY_QUOTA: usize = 0x19;
pub const IRP_MJ_SET_QUOTA: usize = 0x1a;
pub const IRP_MJ_PNP: usize = 0x1b;
pub const IRP_MJ_PNP_POWER: usize = IRP_MJ_PNP; // Obsolete....
pub const IRP_MJ_MAXIMUM_FUNCTION: usize = 0x1b;

// NT_ASSERT(Irp->CurrentLocation <= Irp->StackCount);
// Irp->CurrentLocation++;
// Irp->Tail.Overlay.CurrentStackLocation++;
pub unsafe fn IoSkipCurrentIrpStackLocation(irp: *mut IRP) {
    let irp = &mut *irp;
    assert!(irp.CurrentLocation <= irp.StackCount);

    irp.CurrentLocation += 1;

    let mut stack_location = &mut *irp
        .Tail
        .Overlay_mut()
        .__bindgen_anon_2
        .__bindgen_anon_1
        .CurrentStackLocation_mut();
    *stack_location = stack_location.wrapping_offset(1);
}

pub fn IoGetNextIrpStackLocation(pirp: PIRP) -> PIO_STACK_LOCATION {
    unsafe {
        return (&mut *pirp)
            .Tail
            .Overlay()
            .__bindgen_anon_2
            .__bindgen_anon_1
            .CurrentStackLocation()
            .wrapping_sub(1);
    }
}

pub unsafe fn IoSetCompletionRoutine(
    Irp: PIRP,
    CompletionRoutine: PIO_COMPLETION_ROUTINE,
    Context: PVOID,
    InvokeOnSuccess: bool,
    InvokeOnError: bool,
    InvokeOnCancel: bool,
) {
    let irp_sp = &mut *IoGetNextIrpStackLocation(Irp);
    irp_sp.CompletionRoutine = CompletionRoutine;
    irp_sp.Context = Context;
    irp_sp.Control = 0;

    if InvokeOnSuccess {
        irp_sp.Control |= SL_INVOKE_ON_SUCCESS;
    }

    if InvokeOnError {
        irp_sp.Control |= SL_INVOKE_ON_ERROR;
    }

    if InvokeOnCancel {
        irp_sp.Control |= SL_INVOKE_ON_CANCEL;
    }
}