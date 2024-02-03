use kapi_kmodule::ModuleExport;
use utils_imports::{
    dynamic_import_table,
    provider::SystemExport,
};
use vtk_wsk_sys::{
    IN6_ADDR,
    IN_ADDR,
    PWSK_CLIENT_NPI,
    PWSK_PROVIDER_CHARACTERISTICS,
    PWSK_PROVIDER_NPI,
    PWSK_REGISTRATION,
};
use winapi::shared::ntdef::NTSTATUS;

type RtlIpv4AddressToStringExA = unsafe extern "C" fn(
    Address: &IN_ADDR,
    Port: u16,
    Buffer: *mut u8,
    BufferLength: &mut u32,
) -> NTSTATUS;

type RtlIpv6AddressToStringExA = unsafe extern "C" fn(
    Address: &IN6_ADDR,
    ScopeId: u32,
    Port: u16,
    Buffer: *mut u8,
    BufferLength: &mut u32,
) -> NTSTATUS;

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
        pub RtlIpv4AddressToStringExA: RtlIpv4AddressToStringExA = SystemExport::new(obfstr!("RtlIpv4AddressToStringExA")),
        pub RtlIpv6AddressToStringExA: RtlIpv6AddressToStringExA = SystemExport::new(obfstr!("RtlIpv6AddressToStringExA")),

        pub WskRegister: WskRegister = ModuleExport::new("NETIO.SYS", "WskRegister"),
        pub WskDeregister: WskDeregister = ModuleExport::new("NETIO.SYS", "WskDeregister"),
        pub WskCaptureProviderNPI: WskCaptureProviderNPI = ModuleExport::new("NETIO.SYS", "WskCaptureProviderNPI"),
        pub WskReleaseProviderNPI: WskReleaseProviderNPI = ModuleExport::new("NETIO.SYS", "WskReleaseProviderNPI"),
        pub WskQueryProviderCharacteristics: WskQueryProviderCharacteristics = ModuleExport::new("NETIO.SYS", "WskQueryProviderCharacteristics"),
    }
}
