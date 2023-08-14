// Initial idea: https://github.com/cs1ime/sehcall/tree/main
// Modified for Valthruns use cases.
use core::{arch::{global_asm, asm}, sync::atomic::{AtomicU64, Ordering}};

use alloc::string::ToString;
use anyhow::Context;
use obfstr::obfstr;
use valthrun_driver_shared::ByteSequencePattern;

use crate::{kapi::KModule, offsets::NtOffsets};

use super::SEHException;

// RCX -> SEH function
// RDX -> Callback
// R8 -> Callback arg
global_asm!(r#"
.global _seh_invoke
_seh_invoke:

push rbx
push rsi
push rdi
push rbp
push r10
push r11
push r12
push r13
push r14
push r15
sub rsp, 0x50

call _inner

add rsp, 0x50
pop r15
pop r14
pop r13
pop r12
pop r11
pop r10
pop rbp
pop rdi
pop rsi
pop rbx
ret

_inner:
push rcx
mov rcx, r8
jmp rdx
"#);

extern "system" {
    fn _seh_invoke(seh_wrapper: u64, callback: u64, callback_arg: u64) -> u32;
}

unsafe extern "system" fn seh_callback<F: FnMut()>(closure: *mut ()) {
    let closure = unsafe { &mut *(closure as *mut F) };
    closure();

    /*
     * Setting the return value to zero.
     * KdpSysWriteMsr + 0x0F jmp short loc_5827B3
     * KdpSysWriteMsr + 0xXX mov eax, r8d
     * KdpSysWriteMsr + 0xXX retn
     */
    asm!("mov r8, 0x0");
}

static SEH_TARGET: AtomicU64 = AtomicU64::new(0);

/// Setup SEH
pub fn setup_seh() -> anyhow::Result<()> {
    let kernel_base = KModule::find_by_name(obfstr!("ntoskrnl.exe"))?
        .with_context(|| obfstr!("could not find kernel base").to_string())?;
    
    let pattern = ByteSequencePattern::parse("E8 ? ? ? ? 89 45 EF E9")
        .with_context(|| obfstr!("could not compile KdpSysWriteMsr pattern").to_string())?;

    let seh_target: u64 = NtOffsets::locate_function(
        &kernel_base, obfstr!("KdpSysWriteMsr"), 
        &pattern, 0x01, 0x05
    )?;

    SEH_TARGET.store(seh_target + 0x0F, Ordering::Relaxed);
    Ok(())
}

/// Executes a function in a structure-exception-handled block.<br>
/// This is useful for executing code that may throw an exception, and crash
/// the program. (such as a SEGFAULT)<br><br>
///
/// # Arguments
/// * `closure` - The procedure to execute in the exception-handling block.
///
/// # Returns
/// Some if no exception was thrown, None if an exception was thrown.
/// 
/// ATTENTION:
/// Currently returns SEHException::AccessViolation for every exception due to the SEH implementation logic!
pub fn try_seh<F>(mut closure: F) -> Result<(), SEHException>
    where F: FnMut(),
{
    let seh_target = SEH_TARGET.load(Ordering::Relaxed);
    if seh_target == 0 {
        #[inline(never)]
        fn log_warn() {
            log::warn!("{}", obfstr!("try_seh called, but SEH not yet initialized."));
        }
        log_warn();

        /* TODO: Return another excetion like not initialized or something... */
        return Err(SEHException::AccessViolation);
    }
    let closure_ptr = &mut closure as *mut _  as u64;

    let result = unsafe { _seh_invoke(seh_target, seh_callback::<F> as u64, closure_ptr) };
    if result == 0 { Ok(()) } else { Err(SEHException::AccessViolation) }
}