use alloc::string::String;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct MetricsReport<'a> {
    /// Unique session id for this session.
    pub session_id: &'a str,

    /// Device info
    pub device_info: &'a DeviceInfo,

    /// Entries for the report
    pub entries: &'a [MetricsEntry],
}

#[derive(Debug, Serialize)]
pub struct MetricsEntry {
    /// timestamp is a count of 100-nanosecond intervals since January 1, 1601
    pub timestamp: u64,

    /// PCs uptime in counts of 100-nanoseconds
    pub uptime: u64,

    /// Identifyer for the type of report
    pub report_type: String,

    /// User generated payload.
    pub payload: String,
}

#[derive(Debug, Serialize)]
pub struct DeviceInfo {

}