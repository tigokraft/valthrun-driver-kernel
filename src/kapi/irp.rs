use winapi::{
    km::wdm::{
        IO_PRIORITY::IO_NO_INCREMENT,
        IRP,
    },
    shared::ntdef::NTSTATUS,
};

use crate::imports::GLOBAL_IMPORTS;

pub trait IrpEx {
    fn complete_request(&mut self, status: NTSTATUS) -> NTSTATUS;
}

impl IrpEx for IRP {
    fn complete_request(&mut self, status: NTSTATUS) -> NTSTATUS {
        let imports = GLOBAL_IMPORTS.unwrap();

        self.IoStatus.Information = status as usize;
        unsafe { (imports.IoCompleteRequest)(self, IO_NO_INCREMENT) };
        return status;
    }
}
