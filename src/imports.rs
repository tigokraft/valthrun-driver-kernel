use winapi::shared::ntdef::NTSTATUS;

use crate::{
    dynamic_import_table,
    util::imports::SystemExport,
    wsk::sys::{
        IN6_ADDR,
        IN_ADDR,
        _EPROCESS,
        _KTHREAD,
    },
};

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

type KeQuerySystemTimePrecise = unsafe extern "C" fn(CurrentTime: *mut u64) -> ();
type KeQueryTimeIncrement = unsafe extern "C" fn() -> u32;

type RtlRandomEx = unsafe extern "C" fn(Seed: *mut u32) -> u32;

type KeGetCurrentThread = unsafe extern "C" fn() -> *mut _KTHREAD;
type PsGetCurrentProcess = unsafe extern "C" fn() -> *mut _EPROCESS;
dynamic_import_table! {
    pub imports GLOBAL_IMPORTS {

        pub RtlIpv4AddressToStringExA: RtlIpv4AddressToStringExA = SystemExport::new(obfstr::wide!("RtlIpv4AddressToStringExA")),
        pub RtlIpv6AddressToStringExA: RtlIpv6AddressToStringExA = SystemExport::new(obfstr::wide!("RtlIpv6AddressToStringExA")),

        pub KeQuerySystemTimePrecise: KeQuerySystemTimePrecise = SystemExport::new(obfstr::wide!("KeQuerySystemTimePrecise")),
        pub KeQueryTimeIncrement: KeQueryTimeIncrement = SystemExport::new(obfstr::wide!("KeQueryTimeIncrement")),

        pub RtlRandomEx: RtlRandomEx = SystemExport::new(obfstr::wide!("RtlRandomEx")),

        pub KeGetCurrentThread: KeGetCurrentThread = SystemExport::new(obfstr::wide!("KeGetCurrentThread")),
        pub PsGetCurrentProcess: PsGetCurrentProcess = SystemExport::new(obfstr::wide!("PsGetCurrentProcess")),
    }
}
