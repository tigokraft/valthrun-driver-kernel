use kapi::Process;
use kdef::ProcessProtectionInformation;
use obfstr::obfstr;
use valthrun_driver_shared::requests::{
    RequestProtectionToggle,
    ResponseProtectionToggle,
};

use crate::process_protection;

/// Get EPROCESS.SignatureLevel offset dynamically
pub fn get_eprocess_signature_level_offset() -> isize {
    let base_address = utils_imports::resolve_system(None, obfstr!("PsGetProcessSignatureLevel"));
    let function_bytes: &[u8] =
        unsafe { core::slice::from_raw_parts(base_address.as_ptr() as *const u8, 20) };

    let slice = &function_bytes[15..17];
    let signature_level_offset = u16::from_le_bytes(slice.try_into().unwrap());

    return signature_level_offset as isize;
}

/// Add process protection
pub fn protect_process(process: &Process) {
    let signature_level_offset = get_eprocess_signature_level_offset();
    let ps_protection = unsafe {
        process
            .eprocess()
            .cast::<u8>()
            .offset(signature_level_offset)
            .cast::<ProcessProtectionInformation>()
    };

    unsafe {
        (*ps_protection).signature_level = 0x3f;
        // We're loading DLLs on demand
        //(*ps_protection).section_signature_level = 0x3f;
        // TODO: Reenable as soon as protection has become optional.
        //       Protection type 2 hinters the user to forcefully terminate the application.
        // (*ps_protection).protection = PSProtection::new()
        //     .with_protection_type(2)
        //     .with_protection_audit(0)
        //     .with_protection_signer(6);
    }
}

pub fn handler_protection_toggle(
    req: &RequestProtectionToggle,
    _res: &mut ResponseProtectionToggle,
) -> anyhow::Result<()> {
    let process = Process::current();
    process_protection::toggle_protection(process.get_id(), req.enabled);

    if req.enabled {
        protect_process(&process);
    }

    Ok(())
}
