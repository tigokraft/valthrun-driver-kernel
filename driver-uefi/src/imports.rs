use utils_imports::{
    dynamic_import_table,
    provider::SystemExport,
};
use winapi::shared::ntdef::{
    NTSTATUS,
    UNICODE_STRING,
};

type IoCreateDriver =
    unsafe extern "system" fn(name: *const UNICODE_STRING, entry: *const ()) -> NTSTATUS;
dynamic_import_table! {
    pub imports GLOBAL_IMPORTS {
        pub IoCreateDriver: IoCreateDriver = SystemExport::new(obfstr!("IoCreateDriver")),
    }
}
