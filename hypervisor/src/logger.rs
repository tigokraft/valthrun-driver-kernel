use alloc::{
    boxed::Box,
    sync::Arc,
};
use core::ptr;

use kdef::DPFLTR_LEVEL;
use obfstr::obfstr;
use spin::Mutex;
use winapi::km::wdm::{
    self,
};

use crate::{
    cpu_states,
    logging::{
        Dpc,
        LogQueue,
    },
    panic_hook::DEBUG_IMPORTS,
};

struct VmxDPCContext {
    queue: Arc<spin::Mutex<LogQueue>>,
    local_queue: LogQueue,
}

impl VmxDPCContext {
    pub fn from_queue(queue: Arc<spin::Mutex<LogQueue>>) -> Self {
        let (message_buffer_size, record_buffer_size) = {
            let queue = queue.lock();
            (queue.message_buffer().len(), queue.record_buffer().len())
        };

        Self {
            queue,
            local_queue: LogQueue::new(message_buffer_size, record_buffer_size),
        }
    }
}

pub struct KernelLogger {
    vmx_queue: Arc<spin::Mutex<LogQueue>>,
    dpc: Dpc,
}

impl KernelLogger {
    pub fn queue(&self) -> &Arc<spin::Mutex<LogQueue>> {
        &self.vmx_queue
    }
}

impl log::Log for KernelLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        if cfg!(debug_assertions) {
            true
        } else {
            metadata.level() <= log::Level::Debug
        }
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let level_prefix = match record.level() {
            log::Level::Trace => "T",
            log::Level::Debug => "D",
            log::Level::Info => "I",
            log::Level::Warn => "W",
            log::Level::Error => "E",
        };

        let is_vmx_root_mode = cpu_states::try_current()
            .map(|state| state.vmx_root_mode)
            .unwrap_or(false);

        if is_vmx_root_mode {
            /* we have to queue the log entry */
            {
                let mut queue = self.vmx_queue.lock();
                queue.enqueue_entry(
                    record.level(),
                    format_args!(
                        "[{}][VMX][{}] {}\0",
                        level_prefix,
                        record.module_path().unwrap_or("default"),
                        record.args()
                    ),
                );
            }

            /* Note: If we're the idispatch level interrupt might be delivered immidiately. */
            self.dpc.enqueue(ptr::null(), ptr::null());
            return;
        }

        let message = alloc::fmt::format(format_args!(
            "[{}][{}] {}\0",
            level_prefix,
            record.module_path().unwrap_or("default"),
            record.args()
        ));
        do_log_entry(record.level(), message.as_ptr());
    }

    fn flush(&self) {}
}

static APP_LOGGER: spin::Mutex<Option<KernelLogger>> = spin::Mutex::new(None);
pub fn create_app_logger() -> &'static KernelLogger {
    let mut logger = APP_LOGGER.lock();
    let logger = logger.get_or_insert_with(|| {
        let vmx_queue = Arc::new(Mutex::new(LogQueue::new(1024 * 128, 1024)));
        let vmx_dpc_context = Box::new(VmxDPCContext::from_queue(vmx_queue.clone()));

        let dpc = Dpc::new(
            dpc_process_queue,
            Box::into_raw(vmx_dpc_context) as *const (),
        );
        KernelLogger { vmx_queue, dpc }
    });

    return unsafe { core::mem::transmute(logger) };
}

extern "system" fn dpc_process_queue(
    _dpc: *const wdm::KDPC,
    deferred_context: *mut u8,
    _system_argument1: *const u8,
    _system_argument2: *const u8,
) {
    let ctx = unsafe { &mut *(deferred_context as *mut VmxDPCContext) };
    {
        /*
         * Attention:
         * If we case a VM-Exit while within this function,
         * we will cause a deadlock as soon the VMX handler wants to write to that queue...
         */
        let mut vmx_queue = ctx.queue.lock();
        core::mem::swap(&mut *vmx_queue, &mut ctx.local_queue);
    }

    for (level, message) in ctx.local_queue.entries() {
        do_log_entry(level, message.as_ptr());
    }

    let (records_overflow_count, buffer_overflow_count) = ctx.local_queue.clear_queue();
    if records_overflow_count > 0 {
        log::warn!(
            "Skipping {} entries due to queue record overflow.",
            records_overflow_count
        );
    }

    if buffer_overflow_count > 0 {
        log::warn!(
            "Skipping {} entries due to message buffer overflow.",
            records_overflow_count
        );
    }
}

fn do_log_entry(level: log::Level, message: *const u8) {
    let imports = match DEBUG_IMPORTS.get() {
        Some(imports) => imports,
        /*
         * Debug imports have not been initialized.
         * To avoid infinite looping, we must avoid initializing them.
         */
        None => return,
    };

    let log_level = match level {
        log::Level::Trace => DPFLTR_LEVEL::TRACE,
        log::Level::Debug => DPFLTR_LEVEL::TRACE,
        log::Level::Info => DPFLTR_LEVEL::INFO,
        log::Level::Warn => DPFLTR_LEVEL::WARNING,
        log::Level::Error => DPFLTR_LEVEL::ERROR,
    };

    unsafe {
        (imports.DbgPrintEx)(
            0,
            log_level as u32,
            obfstr!("[VTHV]%s\n\0").as_ptr(),
            message,
        );
    }
}
