use alloc::{
    ffi::CString,
    format,
};

use kdef::DPFLTR_LEVEL;
use obfstr::obfstr;

use crate::panic_hook::DEBUG_IMPORTS;

pub struct KernelLogger;

impl log::Log for KernelLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        if cfg!(debug_assertions) && false {
            true
        } else {
            if metadata.target().contains(obfstr!("embedded_tls")) {
                metadata.level() <= log::Level::Error
            } else if metadata.target().contains(obfstr!("metrics")) {
                metadata.level() <= log::Level::Info
            } else {
                metadata.level() <= log::Level::Debug
            }
        }
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let imports = match DEBUG_IMPORTS.get() {
            Some(imports) => imports,
            /*
             * Debug imports have not been initialized.
             * To avoid infinite looping, we must avoid initializing them.
             */
            None => return,
        };

        let (level_prefix, log_level) = match record.level() {
            log::Level::Trace => ("T", DPFLTR_LEVEL::TRACE),
            log::Level::Debug => ("D", DPFLTR_LEVEL::TRACE),
            log::Level::Info => ("I", DPFLTR_LEVEL::INFO),
            log::Level::Warn => ("W", DPFLTR_LEVEL::WARNING),
            log::Level::Error => ("E", DPFLTR_LEVEL::ERROR),
        };
        let payload = if cfg!(debug_assertions) {
            format!(
                "[{}][{}] {}",
                level_prefix,
                record.module_path().unwrap_or("default"),
                record.args()
            )
        } else {
            format!("[{}] {}", level_prefix, record.args())
        };
        let payload = if let Ok(payload) = CString::new(payload) {
            payload
        } else {
            CString::new(obfstr!("logging message contains null char")).unwrap()
        };

        unsafe {
            (imports.DbgPrintEx)(
                0,
                log_level as u32,
                obfstr!("[VT]%s\n\0").as_ptr(),
                payload.as_ptr(),
            );
        }
    }

    fn flush(&self) {}
}

pub static APP_LOGGER: KernelLogger = KernelLogger;
