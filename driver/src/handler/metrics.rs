use alloc::string::String;

use anyhow::anyhow;
use obfstr::obfstr;
use valthrun_driver_shared::requests::{
    RequestReportSend,
    ResponseReportSend,
};

use crate::METRICS_CLIENT;

pub fn handler_metrics_record(
    req: &RequestReportSend,
    _res: &mut ResponseReportSend,
) -> anyhow::Result<()> {
    let metrics = match unsafe { &*METRICS_CLIENT.get() } {
        Some(client) => client,
        None => return Ok(()),
    };

    let report_type = unsafe {
        if !seh::probe_read(req.report_type as u64, req.report_type_length, 0x1) {
            anyhow::bail!("{}", obfstr!("probe failed for report_type"));
        }

        core::slice::from_raw_parts(req.report_type, req.report_type_length)
    };
    let report_type = String::from_utf8(report_type.to_vec())
        .map_err(|_| anyhow!("{}", obfstr!("report_type is not utf-8")))?;

    let report_payload = unsafe {
        if !seh::probe_read(req.report_payload as u64, req.report_payload_length, 0x1) {
            anyhow::bail!("{}", obfstr!("probe failed for report_payload"));
        }

        core::slice::from_raw_parts(req.report_payload, req.report_payload_length)
    };
    let report_payload = String::from_utf8(report_payload.to_vec())
        .map_err(|_| anyhow!("{}", obfstr!("report_payload is not utf-8")))?;

    metrics.add_record(report_type, report_payload);
    Ok(())
}
