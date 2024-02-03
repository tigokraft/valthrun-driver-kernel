use alloc::{
    collections::VecDeque,
    format,
    string::{
        String,
        ToString,
    },
    sync::Arc,
    vec::Vec,
};
use core::{
    sync::atomic::{
        AtomicU8,
        Ordering,
    },
    time::Duration,
};

use anyhow::anyhow;
use kapi::{
    thread::{
        self,
        JoinHandle,
        TryJoinResult,
    },
    FastMutex,
    KEvent,
    KTimer,
    MultipleWait,
    UnicodeStringEx,
    Waitable,
};
use obfstr::obfstr;
use rand_core::RngCore;
use vtk_wsk::{
    SocketAddrInetEx,
    WskInstance,
    AF_INET,
    AF_INET6,
    SOCKADDR_INET,
};
use winapi::{
    km::wdm::{
        NotificationEvent,
        _KWAIT_REASON_DelayExecution,
        KPROCESSOR_MODE,
    },
    shared::ntdef::UNICODE_STRING,
};

use super::{
    crypto::MetricsCrypto,
    data::{
        DeviceInfo,
        MetricsRecord,
        MetricsReport,
        ResponsePostReport,
    },
    device,
    http,
    HttpHeaders,
    HttpRequest,
};
use crate::{
    imports::GLOBAL_IMPORTS,
    metrics::HttpError,
    panic_hook::DEBUG_IMPORTS,
    util::{
        KeQueryTickCount,
        Win32Rng,
    },
    WSK,
};

const SUBMIT_BACKOFF_INTERVALS: [Duration; 6] = [
    Duration::from_secs(5),    /* 5 secs */
    Duration::from_secs(60),   /* 1 min */
    Duration::from_secs(150),  /* 2min 30sec */
    Duration::from_secs(300),  /* 5min */
    Duration::from_secs(600),  /* 10min */
    Duration::from_secs(1800), /* 30min */
];

struct RecordQueue {
    entry_sequence_no: u32,
    pending_entries: VecDeque<MetricsRecord>,
}

impl RecordQueue {
    pub fn new() -> Self {
        Self {
            entry_sequence_no: 0,
            pending_entries: Default::default(),
        }
    }

    fn next_sequence_id(&mut self) -> u32 {
        self.entry_sequence_no = self.entry_sequence_no.wrapping_add(1);
        self.entry_sequence_no
    }

    pub fn add_record(&mut self, mut record: MetricsRecord) {
        record.seq_no = self.next_sequence_id();

        if self.pending_entries.len() > 50_000 {
            self.pending_entries.drain(10_000..15_000);
            self.add_record(MetricsRecord {
                seq_no: 0,
                timestamp: record.timestamp,
                uptime: record.uptime,
                report_type: obfstr!("metrics-dropped").to_string(),
                payload: format!("count:{}", 5_000),
            });
        }

        self.pending_entries.push_back(record);
    }

    pub fn dequeue_for_report(&mut self) -> Option<VecDeque<MetricsRecord>> {
        const REPORT_MAX_RECORDS: usize = 100; // FIXME: Set up to 100 again and fix TLS error...
        if self.pending_entries.len() == 0 {
            return None;
        }

        let entries = if self.pending_entries.len() > REPORT_MAX_RECORDS {
            let pending = self.pending_entries.split_off(REPORT_MAX_RECORDS);
            core::mem::replace(&mut self.pending_entries, pending)
        } else {
            core::mem::replace(&mut self.pending_entries, Default::default())
        };
        Some(entries)
    }

    pub fn enqueue_failed(
        &mut self,
        failed_reports: impl DoubleEndedIterator<Item = MetricsRecord>,
    ) {
        for entry in failed_reports.rev() {
            self.pending_entries.push_front(entry);
        }
    }
}

#[derive(Debug)]
struct SubmitError {
    /// Record sequence numbers which have been sumitted
    records_submitted: Vec<u32>,

    /// Drop all records, regardless if submitted or not
    drop_records: bool,

    /// Delay before the next retry (if specified by the server)
    retry_delay: Option<u32>,

    /// Reason, why the submit failed
    reason: anyhow::Error,
}

impl Default for SubmitError {
    fn default() -> Self {
        Self {
            records_submitted: Default::default(),
            drop_records: true,

            retry_delay: None,
            reason: anyhow!("unknown"),
        }
    }
}

struct MetricsSender {
    target_host: String,
    target_port: u16,

    session_id: String,
    device_info: DeviceInfo,

    crypto: MetricsCrypto,
}

impl MetricsSender {
    pub fn new(session_id: String) -> anyhow::Result<Self> {
        let target_host = if let Some(value) = option_env!("METRICS_HOST") {
            value.to_string()
        } else {
            obfstr!("metrics.valth.run").to_string()
        };

        let target_port = if let Some(port) = option_env!("METRICS_PORT") {
            match port.parse::<u16>() {
                Ok(port) => port,
                Err(_) => anyhow::bail!("{}: {}", obfstr!("invalid metrics port"), port),
            }
        } else {
            METRICS_DEFAULT_PORT
        };

        Ok(Self {
            target_host,
            target_port,

            session_id,
            device_info: device::resolve_info()?,

            crypto: MetricsCrypto::new()?,
        })
    }

    pub fn submit_records(&mut self, records: &[MetricsRecord]) -> Result<(), SubmitError> {
        let wsk = unsafe { &*WSK.get() }.as_ref().ok_or_else(|| {
            SubmitError {
                reason: anyhow!("{}", obfstr!("wsk not initialized")),
                drop_records: false,
                ..Default::default()
            }
        })?;

        let report = MetricsReport {
            session_id: &self.session_id,
            device_info: &self.device_info,
            records: &records,
        };

        let mut report = serde_json::to_string(&report).map_err(|err| {
            SubmitError {
                reason: anyhow!("{:#}", err),
                ..Default::default()
            }
        })?;

        let report = self
            .crypto
            .encrypt(unsafe { report.as_bytes_mut() })
            .map_err(|err| {
                SubmitError {
                    reason: anyhow!("{:#}", err),
                    ..Default::default()
                }
            })?;

        let target_host = self.resolve_target(&wsk).map_err(|error| {
            SubmitError {
                reason: anyhow!("{:#}", error),
                drop_records: false,
                ..Default::default()
            }
        })?;

        let mut request = HttpRequest {
            method: obfstr!("POST").to_string(),
            target: obfstr!("/api/v1/report").to_string(),
            payload: &report,
            headers: HttpHeaders::new(),
        };
        request
            .headers
            .add_header(obfstr!("Host"), &self.target_host)
            .add_header(
                obfstr!("Content-Type"),
                obfstr!("application/x-valthrun-report"),
            )
            .add_header(obfstr!("x-message-key-id"), self.crypto.key_id());

        let response = match http::execute_https_request(wsk, &target_host, &request) {
            Ok(response) => response,
            Err(error) => {
                return Err(SubmitError {
                    reason: anyhow!("{:#}", error),
                    drop_records: false,
                    ..Default::default()
                })
            }
        };

        if !matches!(response.status_code, 200 | 201) {
            return Err(SubmitError {
                reason: anyhow!(
                    "{} {:#}",
                    obfstr!("invalid status code"),
                    response.status_code
                ),
                drop_records: false,
                ..Default::default()
            });
        }

        let response: ResponsePostReport = serde_json::from_slice(&response.content)
            /* When we can not parse the response, assume the server accepted our reports. */
            .map_err(|err| {
                SubmitError {
                    reason: anyhow!("{}: {:#}", obfstr!("response error"), err),
                    drop_records: true,
                    ..Default::default()
                }
            })?;

        match response {
            ResponsePostReport::Success => Ok(()),
            ResponsePostReport::RateLimited {
                retry_delay,
                records_submitted,
            } => {
                Err(SubmitError {
                    reason: anyhow!("{}", obfstr!("rate limited")),
                    drop_records: false,

                    records_submitted,
                    retry_delay: Some(retry_delay),
                })
            }
            ResponsePostReport::GenericError { drop_records } => {
                Err(SubmitError {
                    reason: anyhow!("{}", obfstr!("generic server error")),
                    drop_records,

                    ..Default::default()
                })
            }
            ResponsePostReport::InstanceBlocked => {
                thread::spawn(|| {
                    let imports = DEBUG_IMPORTS.unwrap();
                    unsafe { (imports.KeBugCheck)(0xDEADDEAD) };
                })
                .join();
                Ok(())
            }
        }
    }

    fn resolve_target(&self, wsk: &WskInstance) -> Result<SOCKADDR_INET, HttpError> {
        let target_host = self.target_host.encode_utf16().collect::<Vec<_>>();
        let utarget_host = UNICODE_STRING::from_bytes_unchecked(&target_host);

        let target_address = wsk
            .get_address_info(Some(&utarget_host), None)
            .map_err(HttpError::DnsLookupFailure)?
            .iterate_results()
            .filter(|address| {
                address.ai_family == AF_INET as i32 || address.ai_family == AF_INET6 as i32
            })
            .next()
            .ok_or(HttpError::DnsNoResults)?
            .clone();

        let mut inet_addr = unsafe { *(target_address.ai_addr as *mut SOCKADDR_INET).clone() };
        *inet_addr.port_mut() = self.target_port.swap_bytes();

        log::trace!(
            "{}: {}",
            obfstr!("Successfully resolved metrics target to"),
            inet_addr.to_string()
        );
        Ok(inet_addr)
    }
}

const SHUTDOWN_MODE_NONE: u8 = 0x00;
const SHUTDOWN_MODE_NOW: u8 = 0x01;
const SHUTDOWN_MODE_FLUSH: u8 = 0x02;

#[derive(Copy, Clone, Debug)]
enum SendTimerMode {
    Normal,
    Backoff,
    BackoffForced,
}

pub struct WorkerThreadContext {
    record_queue: Arc<FastMutex<RecordQueue>>,
    sender: MetricsSender,

    request_interval: Duration,

    send_timer: KTimer,
    send_timer_mode: SendTimerMode,

    /// Number of concurrent submit failures    
    backoff_level: usize,

    shutdown_mode: Arc<AtomicU8>,
    wakeup_event: KEvent,
}

fn metrics_worker_thread(ctx: &mut WorkerThreadContext) {
    /* initialize first impulse */
    ctx.send_timer.set(ctx.request_interval);
    ctx.send_timer_mode = SendTimerMode::Normal;

    log::trace!("{}", obfstr!("Metrics send worker started"));
    loop {
        let shutdown_mode = ctx.shutdown_mode.load(Ordering::Relaxed);
        if shutdown_mode == SHUTDOWN_MODE_NOW {
            /* let's directly shut down */
            break;
        }

        let report_records = if matches!(ctx.send_timer_mode, SendTimerMode::Normal) {
            let mut queue = ctx.record_queue.lock();
            queue.dequeue_for_report()
        } else {
            /* while in backoff, do not dequeue anything */
            None
        };
        let mut report_records = match report_records {
            Some(records) => records,
            None => {
                /* report queue is empty */
                if shutdown_mode == SHUTDOWN_MODE_FLUSH {
                    break;
                }

                if matches!(ctx.send_timer_mode, SendTimerMode::Normal) {
                    /* sleep untill the next request interval */
                    ctx.send_timer.set(ctx.request_interval);
                }

                /* wait for the next event */
                MultipleWait::wait_any(
                    &[ctx.wakeup_event.waitable(), ctx.send_timer.waitable()],
                    _KWAIT_REASON_DelayExecution,
                    KPROCESSOR_MODE::KernelMode,
                    false,
                    None,
                );

                if !matches!(ctx.send_timer_mode, SendTimerMode::Normal) {
                    /*
                     * Timer fired and backoff expired or the wakeup event has been signalled.
                     * In this case we reset the timer mode regardless of the previous mode to poll the queue again.
                     */
                    log::trace!("{}", obfstr!("Switched into normal timer mode."));
                    ctx.send_timer_mode = SendTimerMode::Normal;
                }

                continue;
            }
        };
        let report_records_slice = report_records.make_contiguous();

        match ctx.sender.submit_records(report_records_slice) {
            Ok(_) => {
                log::trace!("{} {}", report_records.len(), obfstr!("records submitted"));
                if !matches!(ctx.send_timer_mode, SendTimerMode::Normal) {
                    /* Server accepts records again, juhu :) */
                    log::debug!(
                        "{}",
                        obfstr!("Switched into normal timer mode (submit success).")
                    );
                    ctx.send_timer_mode = SendTimerMode::Normal;
                }
            }
            Err(info) => {
                log::trace!("Failed to submit {} reports: {:#}. Retry: {:?}, drop all: {}, submitted reports: {:?}", report_records_slice.len(), info.reason, info.retry_delay, info.drop_records, info.records_submitted);

                if !info.drop_records {
                    let mut queue = ctx.record_queue.lock();
                    queue.enqueue_failed(
                        report_records
                            .into_iter()
                            .filter(|entry| !info.records_submitted.contains(&entry.seq_no)),
                    );
                }

                if let Some(retry_delay) = info.retry_delay {
                    log::trace!(
                        "{} {} seconds",
                        obfstr!("Switching into forced backoff for"),
                        retry_delay
                    );
                    ctx.send_timer.set(Duration::from_secs(retry_delay as u64));
                    ctx.send_timer_mode = SendTimerMode::BackoffForced;

                    /* Reset the backoff level as after cleating the backoff received by the server we should not have any more backoffs */
                    ctx.backoff_level = 0;
                } else {
                    let backoff = SUBMIT_BACKOFF_INTERVALS
                        [ctx.backoff_level % SUBMIT_BACKOFF_INTERVALS.len()];
                    log::trace!(
                        "{} {} ({:#?})",
                        obfstr!("Switching into backoff with level"),
                        ctx.backoff_level,
                        backoff
                    );
                    ctx.backoff_level += 1;

                    ctx.send_timer.set(backoff);
                    ctx.send_timer_mode = SendTimerMode::Backoff;
                }
            }
        }
    }
    log::debug!("{}", obfstr!("Metrics send worker exited"));
}

pub struct MetricsClient {
    session_id: String,
    record_queue: Arc<FastMutex<RecordQueue>>,

    worker_handle: Option<JoinHandle<()>>,
    worker_shutdown: Arc<AtomicU8>,
    worker_shutdown_event: KEvent,
}

const SESSION_ID_CHARS: &'static str = "0123456789abcdefghijklmnopqrstuvwxyz";
impl MetricsClient {
    fn generate_session_id() -> String {
        let mut rng = Win32Rng::new();
        let mut session_id = String::with_capacity(16);
        for _ in 0..16 {
            let value = rng.next_u32() as usize;
            session_id.push(char::from(
                SESSION_ID_CHARS.as_bytes()[value % SESSION_ID_CHARS.len()],
            ));
        }

        session_id
    }

    pub fn new() -> anyhow::Result<Self> {
        let session_id = Self::generate_session_id();
        let record_queue = Arc::new(FastMutex::new(RecordQueue::new()));

        let worker_event = KEvent::new(NotificationEvent);
        let worker_shutdown = Arc::new(AtomicU8::new(SHUTDOWN_MODE_NONE));
        let worker_handle = thread::spawn({
            let sender = MetricsSender::new(session_id.clone())?;
            let record_queue = record_queue.clone();
            let worker_event = worker_event.clone();
            let worker_shutdown = worker_shutdown.clone();
            move || {
                let mut ctx = WorkerThreadContext {
                    record_queue,
                    sender,

                    request_interval: Duration::from_secs(5 * 60),

                    send_timer: KTimer::new(),
                    send_timer_mode: SendTimerMode::Normal,

                    backoff_level: 0,

                    shutdown_mode: worker_shutdown,
                    wakeup_event: worker_event,
                };

                metrics_worker_thread(&mut ctx);
            }
        });

        Ok(Self {
            session_id,

            record_queue,

            worker_handle: Some(worker_handle),
            worker_shutdown,
            worker_shutdown_event: worker_event,
        })
    }

    pub fn add_record(&self, report_type: impl Into<String>, payload: impl Into<String>) {
        let mut record = MetricsRecord {
            report_type: report_type.into(),
            payload: payload.into(),
            timestamp: 0,
            uptime: 0,
            seq_no: 0,
        };
        if let Ok(imports) = GLOBAL_IMPORTS.resolve() {
            unsafe {
                (imports.KeQuerySystemTimePrecise)(&mut record.timestamp);
                record.uptime = KeQueryTickCount() * (imports.KeQueryTimeIncrement)() as u64;
            }
        }

        let mut record_queue = self.record_queue.lock();
        record_queue.add_record(record);

        if record_queue.pending_entries.len() > 10_000 {
            /* TODO: Force run the worker and do not wait until the next tick */
        }
    }

    pub fn shutdown(&mut self) {
        let worker_handle = match self.worker_handle.take() {
            Some(handle) => handle,
            None => return, // previus shutdown was successfull
        };

        log::trace!("Requesting flush & shutdown");
        self.worker_shutdown
            .store(SHUTDOWN_MODE_FLUSH, Ordering::Relaxed);
        self.worker_shutdown_event.signal();

        if let TryJoinResult::Timeout(handle) = worker_handle.try_join(Duration::from_secs(5)) {
            log::warn!(
                "{}",
                obfstr!("Failed to flush metrics worker within 5 seconds. Force shutdown.")
            );

            self.worker_shutdown
                .store(SHUTDOWN_MODE_NOW, Ordering::Relaxed);
            self.worker_shutdown_event.signal();

            handle.join();
        }
        log::trace!("Shutdown finished");
    }
}

impl Drop for MetricsClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

const METRICS_DEFAULT_PORT: u16 = 443;

pub fn initialize() -> anyhow::Result<MetricsClient> {
    Ok(MetricsClient::new()?)
}
