use winapi::{
    km::wdm::{
        IO_PRIORITY::IO_NO_INCREMENT,
        IRP,
        PIRP,
    },
    shared::ntdef::NTSTATUS,
};

use crate::imports::{
    IoAllocateIrp,
    IoCancelIrp,
    IoCompleteRequest,
    IoFreeIrp,
};

pub trait IrpEx {
    fn cancel_request(&mut self);
    fn complete_request(&mut self, status: NTSTATUS) -> NTSTATUS;

    fn allocate(stack_size: i8, charge_quota: bool) -> Option<PIRP>;
    fn free(&mut self);
}

impl IrpEx for IRP {
    fn cancel_request(&mut self) {
        unsafe { IoCancelIrp(self) };
    }

    fn complete_request(&mut self, status: NTSTATUS) -> NTSTATUS {
        self.IoStatus.Information = status as usize;
        unsafe { IoCompleteRequest(self, IO_NO_INCREMENT) };
        return status;
    }

    fn allocate(stack_size: i8, charge_quota: bool) -> Option<PIRP> {
        let irp = unsafe { IoAllocateIrp(stack_size, charge_quota) };
        if irp.is_null() {
            None
        } else {
            Some(irp)
        }
    }

    fn free(&mut self) {
        unsafe { IoFreeIrp(self) };
    }
}
