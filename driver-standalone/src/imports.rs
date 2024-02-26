use utils_imports::{
    dynamic_import_table,
    provider::SystemExport,
};
use winapi::{
    km::wdm::KIRQL,
    shared::ntdef::{
        NTSTATUS,
        UNICODE_STRING,
    },
};

type IoCreateDriver =
    unsafe extern "system" fn(name: *const UNICODE_STRING, entry: *const ()) -> NTSTATUS;
type KeGetCurrentIrql = unsafe extern "system" fn() -> KIRQL;
dynamic_import_table! {
    pub imports GLOBAL_IMPORTS {
        pub IoCreateDriver: IoCreateDriver = SystemExport::new(obfstr!("IoCreateDriver")),
        pub KeGetCurrentIrql: KeGetCurrentIrql = SystemExport::new(obfstr!("KeGetCurrentIrql")),
    }
}
