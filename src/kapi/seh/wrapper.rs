// Initial idea: https://github.com/cs1ime/sehcall/tree/main
// Modified for Valthruns use cases.
use core::{arch::global_asm, sync::atomic::{AtomicU64, Ordering}};

use alloc::string::ToString;
use anyhow::Context;
use obfstr::obfstr;
use valthrun_driver_shared::ByteSequencePattern;

use crate::{kapi::KModule, offsets::NtOffsets};

#[repr(C)]
struct SehInvokeInfo {
    seh_target: u64,
    callback: u64,
    callback_a1: u64
}

// RCX -> SehInvokeInfo
// RDX -> Callback A2
// R8 -> Callback A3
// R9 -> Callback A4
global_asm!(r#"
// .btext is at the beginning on the code section
.section .btext
.global _seh_invoke
_seh_invoke:
mov rax, [rsp]
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
sub rsp, 0x28

mov [rsp], rax
call _inner

add rsp, 0x28
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
mov r10, rcx
push [r10]
mov rcx, [r10+0x10]
jmp [r10 + 0x08]
"#);

extern "system" {
    fn _seh_invoke(info: *const SehInvokeInfo, callback_a2: u64, callback_a3: u64, callback_a4: u64) -> u32;
}

static SEH_TARGET: AtomicU64 = AtomicU64::new(0);
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

pub unsafe fn seh_invoke(callback: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> bool {
    let seh_target = SEH_TARGET.load(Ordering::Relaxed);
    if seh_target == 0 {
        #[inline(never)]
        fn log_warn() {
            log::warn!("{}", obfstr!("try_seh called, but SEH not yet initialized."));
        }
        log_warn();

        return false;
    }

    let info = SehInvokeInfo {
        seh_target,
        callback,
        callback_a1: a1
    };

    let result = unsafe {
        _seh_invoke(&info, a2, a3, a4)
    };
    result != 0xC000000E
}

// Attempt to locate the exception directory and manually add it.
// This will most likely call PG to fail...
//
// #[repr(C, align(1))]
// #[derive(Clone, Copy)]
// struct RuntimeFunction {
//     function_start: u32,
//     function_end: u32,
//     unwind_info: u32,
// }
// const _: [(); 0xC] = [(); size_of::<RuntimeFunction>()];

// /// Check if we might have a valid exception directory based
// /// off the first 16 entries. Every section should contain at least
// /// 0xC0 valid bytes therefore we should be fine.
// fn is_maybe_exception_directory(target: *const ()) -> bool {
//     let runtime_functions = unsafe {
//         core::slice::from_raw_parts(
//             target.cast::<RuntimeFunction>(), 
//             0x0F
//         )
//     };

//     let mut current_base = runtime_functions[0];
//     for index in 1..runtime_functions.len() {
//         let current_function = runtime_functions[index];
//         if current_function.function_start < current_base.function_end {
//             /* Entries should be ordered */
//             return false;
//         }

        
//         if (current_function.function_start - current_base.function_end) > 0x1000 {
//             /* Unexpected gap */
//             return false;
//         }

//         current_base = current_function;
//     }

//     true
// }

// /// Setup SEH
// pub fn setup_seh() -> anyhow::Result<()> {
//     /* Section size **must** be a number of power two! */
//     let section_size = 1 << 12;
//     let current_section = (_seh_invoke as u64) & !(section_size - 1);
//     let image_base = current_section - 0x1000; /* PE header might not be present but still taken into account when doing offsets */
//     log::debug!("Base {:X} | Img Base at {:X}", current_section, current_section - 0x1000);
    
//     /* When no exception table is found we might have to increase the search range... */
//     let exception_table = (1..0x100)
//         .map(|index| (current_section + index * section_size) as *const ())
//         .find(|entry| is_maybe_exception_directory(entry.clone()));

//     let exception_table = match exception_table {
//         Some(address) => address as u64,
//         None => anyhow::bail!("failed to locate exception table")
//     };

//     log::debug!("Exception table at {:X} ({:X})", exception_table, exception_table - image_base);

//     unsafe {
//         // let result = RtlAddFunctionTable(exception_table as *const _, 0x91, image_base);
//         // log::debug!("Add result: {:#}", result);
//     }

//     unsafe {
//         asm!("int3");
//     }
//     Ok(())
// }