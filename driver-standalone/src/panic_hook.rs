use core::panic::PanicInfo;

use obfstr::obfstr;

use crate::imports::{
    DbgBreakPoint,
    KeBugCheck,
};

const BUGCHECK_CODE_RUST_PANIC: u32 = 0xC0210001;
const BUGCHECK_CODE_CXX_FRAME: u32 = 0xC0210002;

static mut PANIC_INITIATED: bool = false;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        if PANIC_INITIATED {
            /* we encountered a panic within the panic handler */
            core::intrinsics::abort();
        }

        /*
         * Prevent stack unwinding from an endless loop due to __CxxFrameHandler3
         */
        PANIC_INITIATED = true;
    }

    log::error!("{} {}", obfstr!("Driver"), info);
    log::error!("{}", obfstr!("Trigger BugCheck"));
    unsafe {
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
pub unsafe extern "C" fn __CxxFrameHandler3() {
    if unsafe { PANIC_INITIATED } {
        return;
    }

    log::error!(
        "{}",
        obfstr!("__CxxFrameHandler3 has been called. This should no occur.")
    );
    unsafe {
        DbgBreakPoint();
        KeBugCheck(BUGCHECK_CODE_CXX_FRAME);
    }
}
