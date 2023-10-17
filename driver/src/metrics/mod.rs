mod data;
mod error;
mod http;

use alloc::{
    collections::VecDeque,
    string::{
        String,
        ToString,
    },
    sync::Arc,
    vec::Vec,
};

use anyhow::Context;
pub use error::*;
pub use http::*;
use kapi::{KEvent, FastMutex, thread::{JoinHandle, self}, UnicodeStringEx};
use obfstr::obfstr;
use winapi::{
    km::wdm::NotificationEvent,
    shared::ntdef::UNICODE_STRING,
};

use self::data::{
    DeviceInfo,
    MetricsEntry,
    MetricsReport,
};
use crate::{
    imports::GLOBAL_IMPORTS,
    util::KeQueryTickCount,
    wsk::{
        sys::{
            AF_INET,
            AF_INET6,
            SOCKADDR_INET,
        },
        SocketAddrInetEx,
        WskInstance,
    },
    WSK,
};

enum SubmitResult {
    Success,
}

struct WorkerContext {
    session_id: String,
    device_info: DeviceInfo,

    entry_sequence_no: u32,
    pending_entries: VecDeque<MetricsEntry>,
    shutdown: bool,
}

impl WorkerContext {
    fn executor(event: KEvent, context: Arc<FastMutex<Self>>) {
        loop {
            let mut locked_context = context.lock();
            if locked_context.shutdown {
                break;
            }

            if locked_context.pending_entries.is_empty() {
                /* no pending entries, wait for next event */
                drop(locked_context);
                event.wait_for(None);
                continue;
            }

            let (report, entries) = match locked_context.create_report_payload() {
                Ok(data) => data,
                Err(error) => {
                    log::warn!("{}: {}", obfstr!("Failed to create metrics report"), error);
                    continue;
                }
            };
            drop(locked_context);

            let reenqueue = match Self::send_report(&report) {
                Ok(SubmitResult::Success) => false,
                //Ok(result) => true,
                Err(error) => {
                    log::warn!(
                        "{}: {}. {}",
                        obfstr!("Failed to submit metrics report"),
                        error,
                        obfstr!("Reenqueue and try again later.")
                    );
                    // FIXME: Update next send timestamp.
                    true
                }
            };

            if reenqueue {
                let mut locked_context = context.lock();
                for entry in entries.into_iter().rev() {
                    locked_context.pending_entries.push_front(entry);
                }
            }
        }

        log::trace!("Metrics worker exited");
    }

    pub fn send_report(report: &str) -> anyhow::Result<SubmitResult> {
        let wsk = unsafe { &*WSK.get() };
        let wsk = wsk
            .as_ref()
            .with_context(|| obfstr!("missing wsk instance").to_string())?;

        let (metrics_host, server_address) = resolve_metrics_target(wsk)
            .map_err(|err| anyhow::anyhow!("{}: {:#}", obfstr!("failed to resolve target"), err))?;

        let request = HttpRequest {
            host: &metrics_host,
            target: "/report",
            payload: report.as_bytes(),
        };
        match http::execute_https_request(wsk, &server_address, &request) {
            Ok(response) => {
                /* FIXME: Inspect http response code & json status value */
                log::debug!("Report send with status code {}", response.status_code);
                Ok(SubmitResult::Success)
            }
            Err(error) => anyhow::bail!("submit: {:#}", error),
        }
    }

    fn create_report_payload(&mut self) -> anyhow::Result<(String, Vec<MetricsEntry>)> {
        let entries = self
            .pending_entries
            .drain(0..self.pending_entries.len().min(100))
            .collect::<Vec<_>>();

        let report = MetricsReport {
            session_id: &self.session_id,
            device_info: &self.device_info,
            entries: &entries,
        };

        let estiamted_report_byte_size = 0
            + report.session_id.len()
            + report
                .entries
                .iter()
                .map(|entry| entry.payload.len() + entry.report_type.len() + 128)
                .sum::<usize>()
            + 4096;

        let mut buffer = Vec::new();
        buffer.reserve(estiamted_report_byte_size);

        for _ in 0..1000 {
            unsafe { buffer.set_len(buffer.capacity()) };
            match serde_json_core::to_slice(&report, &mut buffer) {
                Ok(length) => {
                    unsafe { buffer.set_len(length) };
                    let payload = String::from_utf8(buffer)
                        .map_err(|_| anyhow::anyhow!("output contains null characters"))?;
                    return Ok((payload, entries));
                }
                Err(_) => {
                    /* buffer too small, allow additional bytes */
                    buffer.reserve(8192);
                }
            }
        }

        anyhow::bail!(
            "{}",
            obfstr!("failed to allocate big enough buffer for the final report")
        )
    }
}

pub struct MetricsClient {
    session_id: String,

    worker_context: Arc<FastMutex<WorkerContext>>,
    worker_handle: Option<JoinHandle<()>>,
    worker_event: KEvent,
}

const SESSION_ID_CHARS: &'static str = "0123456789abcdefghijklmnopqrstuvwxyz";
impl MetricsClient {
    fn generate_session_id() -> String {
        let imports = GLOBAL_IMPORTS.resolve().unwrap();
        let mut seed = {
            let mut buffer = 0;
            unsafe { (imports.KeQuerySystemTimePrecise)(&mut buffer) };
            buffer as u32
        };

        let mut session_id = String::with_capacity(16);
        for _ in 0..16 {
            let value = unsafe { (imports.RtlRandomEx)(&mut seed) } as usize;
            session_id.push(char::from(
                SESSION_ID_CHARS.as_bytes()[value % SESSION_ID_CHARS.len()],
            ));
        }

        session_id
    }

    pub fn new() -> Self {
        let session_id = Self::generate_session_id();
        let worker_context = WorkerContext {
            entry_sequence_no: 0,

            device_info: DeviceInfo {},
            session_id: session_id.clone(),
            pending_entries: Default::default(),

            shutdown: false,
        };
        let worker_context = Arc::new(FastMutex::new(worker_context));
        let worker_event = KEvent::new(NotificationEvent);
        let worker_handle = thread::spawn({
            let worker_event = worker_event.clone();
            let worker_context = worker_context.clone();
            move || WorkerContext::executor(worker_event, worker_context)
        });

        Self {
            session_id,

            worker_context,
            worker_handle: Some(worker_handle),
            worker_event,
        }
    }

    pub fn add_record(&self, report_type: String, payload: String) {
        let mut entry = MetricsEntry {
            payload,
            report_type,
            timestamp: 0,
            uptime: 0,
            seq_no: 0,
        };
        if let Ok(imports) = GLOBAL_IMPORTS.resolve() {
            unsafe {
                (imports.KeQuerySystemTimePrecise)(&mut entry.timestamp);
                entry.uptime = KeQueryTickCount() * (imports.KeQueryTimeIncrement)() as u64;
            }
        }

        {
            let mut worker_context = self.worker_context.lock();
            worker_context.entry_sequence_no = worker_context.entry_sequence_no.wrapping_add(1);
            entry.seq_no = worker_context.entry_sequence_no;
            worker_context.pending_entries.push_back(entry);
        }
        self.worker_event.signal();
    }

    pub fn shutdown(&mut self) {
        let worker_handle = match self.worker_handle.take() {
            Some(handle) => handle,
            None => return, // previus shutdown was successfull
        };

        /* FIXME: Flush pending entries! */
        log::trace!("Requesting shutdown");
        self.worker_context.lock().shutdown = true;
        self.worker_event.signal();
        worker_handle.join();
        log::debug!("Shutdown successfull");
    }
}

impl Drop for MetricsClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

const METRICS_DEFAULT_PORT: u16 = 80;
fn resolve_metrics_target(wsk: &WskInstance) -> Result<(String, SOCKADDR_INET), HttpError> {
    let target_host = if let Some(override_value) = option_env!("METRICS_HOST") {
        String::from(override_value)
            .encode_utf16()
            .collect::<Vec<_>>()
    } else {
        obfstr::wide!("metrics.valth.run")
            .iter()
            .cloned()
            .collect::<Vec<_>>()
    };
    let utarget_domain = UNICODE_STRING::from_bytes_unchecked(&target_host);

    let target_address = wsk
        .get_address_info(Some(&utarget_domain), None)
        .map_err(HttpError::DnsLookupFailure)?
        .iterate_results()
        .filter(|address| {
            address.ai_family == AF_INET as i32 || address.ai_family == AF_INET6 as i32
        })
        .next()
        .ok_or(HttpError::DnsNoResults)?
        .clone();

    let mut inet_addr = unsafe { *(target_address.ai_addr as *mut SOCKADDR_INET).clone() };
    if let Some(port) = option_env!("METRICS_PORT") {
        *inet_addr.port_mut() = match port.parse::<u16>() {
            Ok(port) => port.swap_bytes(),
            Err(_) => {
                log::warn!(
                    "{}",
                    obfstr!("Failed to parse custom metrics port. Using default port.")
                );
                METRICS_DEFAULT_PORT.swap_bytes()
            }
        };
    } else {
        *inet_addr.port_mut() = METRICS_DEFAULT_PORT.swap_bytes();
    }

    log::trace!(
        "{}: {}",
        obfstr!("Successfully resolved metrics target to"),
        inet_addr.to_string()
    );
    Ok((String::from_utf16_lossy(&target_host), inet_addr))
}

pub fn initialize() -> anyhow::Result<MetricsClient> {
    Ok(MetricsClient::new())
}
