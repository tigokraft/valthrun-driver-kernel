use winapi::shared::ntdef::NTSTATUS;

use super::sys::{
    PWSK_CLIENT_NPI,
    PWSK_PROVIDER_CHARACTERISTICS,
    PWSK_PROVIDER_NPI,
    PWSK_REGISTRATION,
};
use crate::{
    dynamic_import_table,
    util::imports::ModuleExport,
};

type WskRegister = unsafe extern "C" fn(
    WskClientNpi: PWSK_CLIENT_NPI,
    WskRegistration: PWSK_REGISTRATION,
) -> NTSTATUS;

type WskDeregister = unsafe extern "C" fn(WskRegistration: PWSK_REGISTRATION);

type WskCaptureProviderNPI = unsafe extern "C" fn(
    WskRegistration: PWSK_REGISTRATION,
    WaitTimeout: u32,
    WskProviderNpi: PWSK_PROVIDER_NPI,
) -> NTSTATUS;

type WskReleaseProviderNPI = unsafe extern "C" fn(WskRegistration: PWSK_REGISTRATION);

type WskQueryProviderCharacteristics = unsafe extern "C" fn(
    WskRegistration: PWSK_REGISTRATION,
    WskProviderCharacteristics: PWSK_PROVIDER_CHARACTERISTICS,
) -> NTSTATUS;

dynamic_import_table! {
    pub imports WSK_IMPORTS {
        pub WskRegister: WskRegister = ModuleExport::new("NETIO.SYS", "WskRegister"),
        pub WskDeregister: WskDeregister = ModuleExport::new("NETIO.SYS", "WskDeregister"),
        pub WskCaptureProviderNPI: WskCaptureProviderNPI = ModuleExport::new("NETIO.SYS", "WskCaptureProviderNPI"),
        pub WskReleaseProviderNPI: WskReleaseProviderNPI = ModuleExport::new("NETIO.SYS", "WskReleaseProviderNPI"),
        pub WskQueryProviderCharacteristics: WskQueryProviderCharacteristics = ModuleExport::new("NETIO.SYS", "WskQueryProviderCharacteristics"),
    }
}
