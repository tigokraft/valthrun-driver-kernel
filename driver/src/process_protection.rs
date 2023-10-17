use alloc::{
    boxed::Box,
    string::ToString,
    vec::Vec,
};

use anyhow::{
    anyhow,
    Context,
};
use kapi::{FastMutex, Process, OBJECT_TYPE_IMPORT, UnicodeStringEx, NTStatusEx};
use kapi_kmodule::KModule;
use kdef::{_OB_PRE_OPERATION_INFORMATION, _OB_PRE_CREATE_HANDLE_INFORMATION, _OB_PRE_DUPLICATE_HANDLE_INFORMATION, _OB_OPERATION_REGISTRATION, _OB_CALLBACK_REGISTRATION, OB_OPERATION_HANDLE_DUPLICATE, OB_OPERATION_HANDLE_CREATE, OB_FLT_REGISTRATION_VERSION};
use log::Level;
use obfstr::obfstr;
use once_cell::race::OnceBox;
use valthrun_driver_shared::ByteSequencePattern;
use winapi::shared::ntdef::{
    PVOID,
    UNICODE_STRING,
};

use crate::{imports::GLOBAL_IMPORTS, offsets::get_nt_offsets};


struct ProtectionState {
    ob_registration: PVOID,
    protected_process_ids: Vec<i32>,
}

unsafe impl Send for ProtectionState {}
unsafe impl Sync for ProtectionState {}

static PROCESS_PROTECTION: OnceBox<FastMutex<Option<ProtectionState>>> = OnceBox::new();

fn process_protection_state() -> &'static FastMutex<Option<ProtectionState>> {
    PROCESS_PROTECTION.get_or_init(|| Box::new(FastMutex::new(None)))
}

/*
 * _ctx will point to the method itself as we needed a jump to get here.
 * See ObRegisterCallbacks for more info.
 */
extern "system" fn process_protection_callback(
    _ctx: PVOID,
    info: *const _OB_PRE_OPERATION_INFORMATION,
) -> u32 {
    let info = unsafe { &*info };

    let current_process = Process::current();
    let target_process = Process::from_raw(info.Object, false);

    if current_process.eprocess() == target_process.eprocess() || (info.Flags & 0x01) > 0 {
        /* own attachments and attachments from the kernel are allowed */
        return 0;
    }

    let imports = GLOBAL_IMPORTS.unwrap();
    let target_process_id = unsafe { (imports.PsGetProcessId)(info.Object) };
    if log::log_enabled!(target: "ProcessAttachments", Level::Trace) && false {
        let current_process_name = current_process.get_image_file_name().unwrap_or_default();
        if current_process_name != obfstr!("svchost.exe")
            && current_process_name != obfstr!("WmiPrvSE.exe")
        {
            log::trace!("process_protection_callback. Caller: {:X} ({:?}), Target: {:X} ({:?}) Flags: {:X}, Operation: {:X}", 
                current_process.get_id(), current_process_name,
                target_process_id, target_process.get_image_file_name(),
                info.Flags, info.Operation);
        }
    }

    let is_protected = {
        let context = process_protection_state().lock();
        let context = match context.as_ref() {
            Some(ctx) => ctx,
            None => return 0,
        };

        context.protected_process_ids.contains(&target_process_id)
    };

    if !is_protected {
        /* all is good :) */
        return 0;
    }

    log::debug!("Process 0x{:X} ({}) tries to open a handle to the protected process 0x{:X} ({}) (Operation: 0x{:0<2X})", 
        current_process.get_id(), current_process.get_image_file_name().unwrap_or("[[ error ]]"), 
        target_process.get_id(), target_process.get_image_file_name().unwrap_or("[[ error ]]"), 
        info.Operation
    );

    match info.Operation {
        OB_OPERATION_HANDLE_CREATE => {
            let parameters = unsafe {
                &mut *core::mem::transmute::<_, *mut _OB_PRE_CREATE_HANDLE_INFORMATION>(
                    info.Parameters,
                )
            };

            // SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION
            parameters.DesiredAccess = 0x00100000 | 0x1000;
        }
        OB_OPERATION_HANDLE_DUPLICATE => {
            let parameters = unsafe {
                &mut *core::mem::transmute::<_, *mut _OB_PRE_DUPLICATE_HANDLE_INFORMATION>(
                    info.Parameters,
                )
            };

            // SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION
            parameters.DesiredAccess = 0x00100000 | 0x1000;
        }
        op => log::warn!("Tried to protect {target_process_id:X} but operation {op} unknown."),
    }
    0
}

pub fn toggle_protection(target_process_id: i32, target: bool) {
    let mut context = process_protection_state().lock();
    let context = match context.as_mut() {
        Some(ctx) => ctx,
        None => {
            log::warn!("Tried to protect process, but process protection not yet initialized");
            return;
        }
    };

    if target {
        if !context.protected_process_ids.contains(&target_process_id) {
            context.protected_process_ids.push(target_process_id);
        }

        log::debug!("Enabled process protection for {}", target_process_id);
    } else {
        if let Some(index) = context
            .protected_process_ids
            .iter()
            .position(|id| *id == target_process_id)
        {
            context.protected_process_ids.swap_remove(index);
            log::debug!("Disabled process protection for {}", target_process_id);
        }
    }
}

pub fn finalize() {
    let context = {
        let mut context = process_protection_state().lock();
        match context.take() {
            Some(ctx) => ctx,
            None => return,
        }
    };

    let imports = GLOBAL_IMPORTS.unwrap();
    unsafe {
        (imports.ObUnRegisterCallbacks)(context.ob_registration);
    }
}

#[allow(unused)]
pub fn initialize() -> anyhow::Result<()> {
    let imports = GLOBAL_IMPORTS.unwrap();

    let mut context = process_protection_state().lock();
    if context.is_some() {
        anyhow::bail!("{}", obfstr!("process protection already initialized"));
    }

    let mut reg_handle = core::ptr::null_mut();
    *context = unsafe {
        let pattern = ByteSequencePattern::parse(obfstr!("FF E1"))
            .with_context(|| obfstr!("failed to compile jmp rcx pattern").to_string())?;

        #[allow(non_snake_case)]
        let MmVerifyCallbackFunctionFlags = get_nt_offsets().MmVerifyCallbackFunctionFlags;

        let (jmp_module, jmp_target) = KModule::query_modules()?
            .into_iter()
            .filter(|module| module.file_name.ends_with(".sys"))
            .filter(KModule::is_base_data_valid)
            .find_map(|module| {
                // log::debug!("Scanning {} ({:X} - {:X})", module.file_name, module.base_address, module.base_address + module.module_size);
                let sections = match module.find_code_sections() {
                    Ok(sections) => sections,
                    Err(_) => return None,
                };

                let jmp_target = sections
                    .iter()
                    .filter(|section| section.is_data_valid())
                    .filter(|section| {
                        // log::debug!(" Testing {} at {:X} ({:X} bytes)", section.name, section.raw_data_address(), section.size_of_raw_data);
                        MmVerifyCallbackFunctionFlags(section.raw_data_address() as PVOID, 0x20)
                    })
                    .find_map(|section| {
                        // log::debug!("  Searching pattern");
                        section.find_pattern(&pattern)
                    });

                if let Some(target) = jmp_target {
                    Some((module, target))
                } else {
                    None
                }
            })
            .with_context(|| obfstr!("failed to find any valid ob callback").to_string())?;

        // log::debug!(
        //     "Found callback target in {} at {:X}",
        //     jmp_module.file_path,
        //     jmp_target
        // );

        let mut operation_reg = core::mem::zeroed::<_OB_OPERATION_REGISTRATION>();
        operation_reg.ObjectType = OBJECT_TYPE_IMPORT.unwrap().PsProcessType;
        operation_reg.Operations = OB_OPERATION_HANDLE_CREATE | OB_OPERATION_HANDLE_DUPLICATE;
        operation_reg.PostOperation = None;

        let mut callback_reg = core::mem::zeroed::<_OB_CALLBACK_REGISTRATION>();
        callback_reg.Version = OB_FLT_REGISTRATION_VERSION;
        callback_reg.Altitude = UNICODE_STRING::from_bytes(obfstr::wide!("1")); /* Yes we want to be one of the first */
        callback_reg.OperationRegistration = &operation_reg;
        callback_reg.OperationRegistrationCount = 1;

        // https://www.unknowncheats.me/forum/2350590-post9.html
        operation_reg.PreOperation = Some(core::mem::transmute(jmp_target));
        callback_reg.RegistrationContext = process_protection_callback as PVOID;

        // An anticheat which registers a lowest and highest altitude callback
        // can just reset the desiered permissions (especially with file name filtering).
        // Therefore this "protection" is easily removeable. Anyhow this requires a kernel module!
        (imports.ObRegisterCallbacks)(&callback_reg, &mut reg_handle)
            .ok()
            .map_err(|err| {
                anyhow!(
                    "ObRegisterCallbacks ({:X}) {:X}",
                    operation_reg.PreOperation.unwrap() as usize,
                    err
                )
            })?;

        Some(ProtectionState {
            ob_registration: reg_handle,
            protected_process_ids: Default::default(),
        })
    };

    Ok(())
}
