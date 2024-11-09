use alloc::string::String;

use anyhow::{
    anyhow,
    Context,
};
use obfstr::obfstr;
use valthrun_driver_protocol::command::DriverCommandMetricsReportSend;

use crate::METRICS_CLIENT;

pub fn handler_metrics_record(command: &mut DriverCommandMetricsReportSend) -> anyhow::Result<()> {
    let metrics = match unsafe { &*METRICS_CLIENT.get() } {
        Some(client) => client,
        None => return Ok(()),
    };

    let report_type = command.get_report_type().context("invalid report type")?;
    let report_payload = unsafe {
        if !seh::probe_read(
            command.report_payload as u64,
            command.report_payload_length,
            0x1,
        ) {
            anyhow::bail!("{}", obfstr!("probe failed for report_payload"));
        }

        core::slice::from_raw_parts(command.report_payload, command.report_payload_length)
    };
    let report_payload = String::from_utf8(report_payload.to_vec())
        .map_err(|_| anyhow!("{}", obfstr!("report_payload is not utf-8")))?;

    metrics.add_record(report_type, report_payload);
    Ok(())
}
