use core::arch::asm;

use utils_imports::{
    dynamic_import_table,
    provider::SystemExport,
};

#[allow(non_camel_case_types)]
type PKIPI_BROADCAST_WORKER = unsafe extern "system" fn(context: usize) -> usize;
type KeIpiGenericCall =
    unsafe extern "system" fn(target: PKIPI_BROADCAST_WORKER, context: usize) -> usize;

type KeQueryActiveProcessorCount = unsafe extern "system" fn(ActiveProcessors: *mut u32) -> u32;

dynamic_import_table! {
    imports DYNAMIC_IMPORTS {
        pub KeIpiGenericCall: KeIpiGenericCall = SystemExport::new("KeIpiGenericCall"),
        pub KeQueryActiveProcessorCount: KeQueryActiveProcessorCount = SystemExport::new("KeQueryActiveProcessorCount"),
    }
}
pub fn active_count() -> usize {
    let count =
        unsafe { (DYNAMIC_IMPORTS.unwrap().KeQueryActiveProcessorCount)(core::ptr::null_mut()) };
    count as usize
}

pub fn current() -> usize {
    let result;
    unsafe {
        asm!(
            "xor {out}, {out}",
            "mov {out:l}, gs:0184h",
            out = out(reg) result
        );
    }
    result
}

pub fn run_on_all<F>(target: F)
where
    F: Fn() -> () + Send + Sync,
{
    unsafe extern "system" fn _c_callback<F>(context: usize) -> usize
    where
        F: Fn() -> () + Send + Sync,
    {
        let f: *const F = core::mem::transmute(context);
        (*f)();
        0
    }

    unsafe {
        (DYNAMIC_IMPORTS.unwrap().KeIpiGenericCall)(_c_callback::<F>, &target as *const _ as usize);
    }
}
