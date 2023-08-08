use core::panic::PanicInfo;

use alloc::format;
use winapi::km::wdm::{DbgPrintEx, DbgBreakPoint};

use crate::kdef::{DPFLTR_LEVEL, KeBugCheck};

const BUGCHECK_CODE_RUST_PANIC: u32 = 0xC0210001;
const BUGCHECK_CODE_CXX_FRAME: u32 = 0xC0210002;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        DbgPrintEx(0, DPFLTR_LEVEL::ERROR as u32, format!("[VT] Driver {}.\n\0", info).as_ptr());
        DbgPrintEx(0, DPFLTR_LEVEL::ERROR as u32, "[VT] Trigger BugCheck.\n\0".as_ptr());
        DbgBreakPoint();
        KeBugCheck(BUGCHECK_CODE_RUST_PANIC);
    }
}

/// Explanation can be found here: https://github.com/Trantect/win_driver_example/issues/4
#[export_name = "_fltused"]
static _FLTUSED: i32 = 0;

/// When using the alloc crate it seems like it does some unwinding. Adding this
/// export satisfies the compiler but may introduce undefined behaviour when a
/// panic occurs.
/// 
/// Source: https://github.com/memN0ps/rootkit-rs/blob/da9a9d04b18fea58870aa1dbb71eaf977b923664/driver/src/lib.rs#L32
#[no_mangle]
extern "C" fn __CxxFrameHandler3() -> ! {
    unsafe {
        DbgPrintEx(0, DPFLTR_LEVEL::ERROR as u32, "[VT] __CxxFrameHandler3 has been called. This should no occur.\n\0".as_ptr());
        DbgBreakPoint();
        KeBugCheck(BUGCHECK_CODE_CXX_FRAME);
    }
}