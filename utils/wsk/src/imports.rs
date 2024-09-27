#![allow(non_snake_case)]

use lazy_link::lazy_link;
use vtk_wsk_sys::{
    IN6_ADDR,
    IN_ADDR,
    PWSK_CLIENT_NPI,
    PWSK_PROVIDER_CHARACTERISTICS,
    PWSK_PROVIDER_NPI,
    PWSK_REGISTRATION,
};
use winapi::shared::ntdef::NTSTATUS;

#[lazy_link(resolver = "kapi_kmodule::resolve_import")]
extern "C" {
    pub fn RtlIpv4AddressToStringExA(
        Address: &IN_ADDR,
        Port: u16,
        Buffer: *mut u8,
        BufferLength: &mut u32,
    ) -> NTSTATUS;

    pub fn RtlIpv6AddressToStringExA(
        Address: &IN6_ADDR,
        ScopeId: u32,
        Port: u16,
        Buffer: *mut u8,
        BufferLength: &mut u32,
    ) -> NTSTATUS;
}

#[lazy_link(resolver = "kapi_kmodule::resolve_import", module = "NETIO.SYS")]
extern "C" {
    pub fn WskRegister(
        WskClientNpi: PWSK_CLIENT_NPI,
        WskRegistration: PWSK_REGISTRATION,
    ) -> NTSTATUS;

    pub fn WskDeregister(WskRegistration: PWSK_REGISTRATION);

    pub fn WskCaptureProviderNPI(
        WskRegistration: PWSK_REGISTRATION,
        WaitTimeout: u32,
        WskProviderNpi: PWSK_PROVIDER_NPI,
    ) -> NTSTATUS;

    pub fn WskReleaseProviderNPI(WskRegistration: PWSK_REGISTRATION);

    pub fn WskQueryProviderCharacteristics(
        WskRegistration: PWSK_REGISTRATION,
        WskProviderCharacteristics: PWSK_PROVIDER_CHARACTERISTICS,
    ) -> NTSTATUS;
}
