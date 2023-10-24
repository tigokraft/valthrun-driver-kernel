use alloc::sync::Arc;
use core::{
    sync::atomic::{
        AtomicBool,
        Ordering,
    },
    time::Duration,
};

use kapi::{
    thread::JoinHandle,
    KEvent,
    KTimer,
    MultipleWait,
    Waitable,
};
use winapi::km::wdm::{
    NotificationEvent,
    _KWAIT_REASON_DelayExecution,
    KPROCESSOR_MODE,
};

use super::RECORD_TYPE_DRIVER_HEARTBEAT;
use crate::METRICS_CLIENT;

pub struct MetricsHeartbeat {
    handle: JoinHandle<()>,
    shutdown: KEvent,
    shutdown_flag: Arc<AtomicBool>,
}

impl MetricsHeartbeat {
    pub fn new(interval: Duration) -> Self {
        let shutdown = KEvent::new(NotificationEvent);
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let handle = kapi::thread::spawn({
            let shutdown = shutdown.clone();
            let shutdown_flag = shutdown_flag.clone();
            move || {
                let timer = KTimer::new();
                while !shutdown_flag.load(Ordering::Relaxed) {
                    timer.set(interval);

                    let wait_result = [shutdown.waitable(), timer.waitable()].wait_any(
                        _KWAIT_REASON_DelayExecution,
                        KPROCESSOR_MODE::KernelMode,
                        false,
                        None,
                    );

                    if wait_result.unwrap_or(0) == 0 {
                        /* shutdown event has been raised */
                        continue;
                    }

                    if let Some(client) = unsafe { &*METRICS_CLIENT.get() } {
                        client.add_record(RECORD_TYPE_DRIVER_HEARTBEAT, "");
                    }
                }
            }
        });

        Self {
            handle,
            shutdown,
            shutdown_flag,
        }
    }

    pub fn shutdown(self) {
        self.shutdown_flag.store(true, Ordering::Relaxed);
        self.shutdown.signal();
        self.handle.join();
    }
}
