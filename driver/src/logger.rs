#![allow(static_mut_refs)]

use alloc::{
    ffi::CString,
    format,
};
use core::mem;

use kdef::DPFLTR_LEVEL;
use obfstr::obfstr;
use winapi::shared::ntdef::NTSTATUS;

type DbgPrintEx =
    unsafe extern "C" fn(ComponentId: u32, Level: u32, Format: *const u8, ...) -> NTSTATUS;

pub struct KernelLogger {
    dbg_print_ex: DbgPrintEx,
}

impl log::Log for KernelLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        if cfg!(debug_assertions) {
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
            (self.dbg_print_ex)(
                77, /* DPFLTR_IHVDRIVER_ID */
                log_level as u32,
                obfstr!("[VT]%s\n\0").as_ptr(),
                payload.as_ptr(),
            );
        }
    }

    fn flush(&self) {}
}

static mut APP_LOGGER: Option<KernelLogger> = None;
pub fn get_logger_instance() -> &'static KernelLogger {
    unsafe {
        APP_LOGGER.get_or_insert_with(|| {
            KernelLogger {
                dbg_print_ex: mem::transmute(
                    utils_imports::resolve_system(None, "DbgPrintEx").as_ptr(),
                ),
            }
        })
    }
}
