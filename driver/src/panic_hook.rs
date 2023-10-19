use alloc::format;
use core::{
    cell::SyncUnsafeCell,
    panic::PanicInfo,
};

use kdef::DPFLTR_LEVEL;
use obfstr::obfstr;
use utils_imports::{
    dynamic_import_table,
    provider::SystemExport,
};
use winapi::shared::ntdef::NTSTATUS;

type DbgPrintEx =
    unsafe extern "C" fn(ComponentId: u32, Level: u32, Format: *const u8, ...) -> NTSTATUS;
type DbgBreakPoint = unsafe extern "system" fn();
type KeBugCheck = unsafe extern "system" fn(code: u32) -> !;

dynamic_import_table! {
    /// These imports should not fail!
    pub imports DEBUG_IMPORTS {
        pub DbgPrintEx: DbgPrintEx = SystemExport::new(obfstr!("DbgPrintEx")),
        pub DbgBreakPoint: DbgBreakPoint = SystemExport::new(obfstr!("DbgBreakPoint")),
        pub KeBugCheck: KeBugCheck = SystemExport::new(obfstr!("KeBugCheck")),
    }
}

const BUGCHECK_CODE_RUST_PANIC: u32 = 0xC0210001;
const BUGCHECK_CODE_CXX_FRAME: u32 = 0xC0210002;

static PANIC_INITIATED: SyncUnsafeCell<bool> = SyncUnsafeCell::new(false);

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        /*
         * Prevent stack unwinding from an endless loop due to __CxxFrameHandler3
         */
        *PANIC_INITIATED.get() = true;
    }

    /*
     * We can't resolve imports at this stage as this could resolve in an endless loop.
     * Ether we got imports or not.
     */
    if let Some(imports) = DEBUG_IMPORTS.get() {
        unsafe {
            (imports.DbgPrintEx)(
                0,
                DPFLTR_LEVEL::ERROR as u32,
                format!("{} {}.\n\0", obfstr!("[VT] Driver"), info).as_ptr(),
            );
            (imports.DbgPrintEx)(
                0,
                DPFLTR_LEVEL::ERROR as u32,
                obfstr!("[VT] Trigger BugCheck.\n\0").as_ptr(),
            );
            (imports.DbgBreakPoint)();
            (imports.KeBugCheck)(BUGCHECK_CODE_RUST_PANIC);
        }
    } else {
        /*
         * We can't to anything else...
         */
        core::intrinsics::abort();
    }
}

/// Explanation can be found here: https://github.com/Trantect/win_driver_example/issues/4
#[export_name = "_fltused"]
static _FLTUSED: i32 = 0;

#[no_mangle]
pub extern "C" fn __chkstk() {}

/// When using the alloc crate it seems like it does some unwinding. Adding this
/// export satisfies the compiler but may introduce undefined behaviour when a
/// panic occurs.
///
/// Source: https://github.com/memN0ps/rootkit-rs/blob/da9a9d04b18fea58870aa1dbb71eaf977b923664/driver/src/lib.rs#L32
#[no_mangle]
pub unsafe extern "C" fn __CxxFrameHandler3() {
    if unsafe { *PANIC_INITIATED.get() } {
        return;
    }

    let imports = DEBUG_IMPORTS.unwrap();
    unsafe {
        (imports.DbgPrintEx)(
            0,
            DPFLTR_LEVEL::ERROR as u32,
            obfstr!("[VT] __CxxFrameHandler3 has been called. This should no occur.\n\0").as_ptr(),
        );
        (imports.DbgBreakPoint)();
        (imports.KeBugCheck)(BUGCHECK_CODE_CXX_FRAME);
    }
}
