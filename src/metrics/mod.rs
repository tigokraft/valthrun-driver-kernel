mod error;
mod http;
mod data;

use alloc::{string::{String, ToString}, vec::Vec};
use anyhow::Context;
pub use error::*;
pub use http::*;
use obfstr::obfstr;

use crate::{imports::GLOBAL_IMPORTS, util::KeQueryTickCount, WSK};

use self::data::{MetricsEntry, MetricsReport, DeviceInfo};

pub struct MetricsClient {
    session_id: String,
    pending_entries: Vec<MetricsEntry>,
    device_info: DeviceInfo,
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
            session_id.push(
                char::from(
                    SESSION_ID_CHARS.as_bytes()[value % SESSION_ID_CHARS.len()]
                )
            );
        }

        session_id
    }

    pub fn new() -> Self {
        Self {
            session_id: Self::generate_session_id(),
            pending_entries: Default::default(),
            device_info: DeviceInfo {

            }
        }
    }

    pub fn add_record(&mut self, report_type: String, payload: String) {
        let mut entry = MetricsEntry {
            payload,
            report_type,
            timestamp: 0,
            uptime: 0,
        };
        if let Ok(imports) = GLOBAL_IMPORTS.resolve() {
            unsafe {
                (imports.KeQuerySystemTimePrecise)(&mut entry.timestamp);
                entry.uptime = KeQueryTickCount() * (imports.KeQueryTimeIncrement)() as u64;
            }
        }
        self.pending_entries.push(entry);
    }

    pub fn send_report(&mut self) -> anyhow::Result<()> {
        let wsk = unsafe { &*WSK.get() };
        let wsk = wsk.as_ref().with_context(|| obfstr!("missing wsk instance").to_string())?;

        let (report_payload, _entries) = self.create_report_payload()?;
        match http::send_report(wsk, "/report", &report_payload) {
            Ok(response) => {
                log::debug!("Report send with status code {}", response.status_code);
            },
            Err(error) => {
                /* FIXME: Reenqueue reports! */
                anyhow::bail!("Failed to send report: {:#}", error);
            }
        }
        log::debug!("Report: {}", report_payload);
        Ok(())
    }

    fn create_report_payload(&mut self) -> anyhow::Result<(String, Vec<MetricsEntry>)> {
        let entries = self.pending_entries
            .drain(0..self.pending_entries.len().min(100))
            .collect::<Vec<_>>();

        let report = MetricsReport {
            session_id: &self.session_id,
            device_info: &self.device_info,
            entries: &entries,
        };


        let estiamted_report_byte_size = 0 +
            report.session_id.len() + 
            report.entries.iter().map(|entry| entry.payload.len() + entry.report_type.len() + 128).sum::<usize>() +
            4096;

            
        let mut buffer = Vec::new();
        buffer.reserve(estiamted_report_byte_size);

        for _ in 0..1000 {
            unsafe { buffer.set_len(buffer.capacity()) };
            match serde_json_core::to_slice(&report, &mut buffer) {
                Ok(length) => {
                    unsafe { buffer.set_len(length) };
                    let payload = String::from_utf8(buffer).map_err(|_| anyhow::anyhow!("output contains null characters"))?;
                    return Ok((payload, entries));
                },
                Err(_) => {
                    /* buffer too small, allow additional bytes */
                    buffer.reserve(8192);
                }
            }
        }
        
        anyhow::bail!("{}", obfstr!("failed to allocate big enough buffer for the final report"))
    }
}