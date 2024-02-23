use alloc::{
    boxed::Box,
    vec::Vec,
};
use core::fmt;

use super::BufferFmt;

#[derive(Debug, Copy, Clone)]
pub struct LogQueueEntry {
    level: log::Level,

    message_offset: usize,
    message_length: usize,
}

pub struct LogQueue {
    buffer: Box<[u8]>,
    buffer_offset: usize,
    buffer_overflow_count: usize,

    records: Box<[LogQueueEntry]>,
    records_offset: usize,
    records_overflow_count: usize,
}

impl LogQueue {
    pub fn new(log_capacity: usize, record_capacity: usize) -> Self {
        let mut buffer = Vec::new();
        buffer.resize(log_capacity, 0u8);

        let mut records = Vec::new();
        records.resize_with(record_capacity, || {
            LogQueueEntry {
                level: log::Level::Debug,
                message_offset: 0,
                message_length: 0,
            }
        });

        Self {
            buffer: buffer.into_boxed_slice(),
            buffer_offset: 0,
            buffer_overflow_count: 0,

            records: records.into_boxed_slice(),
            records_offset: 0,
            records_overflow_count: 0,
        }
    }

    pub fn message_buffer(&self) -> &[u8] {
        &self.buffer
    }

    pub fn record_buffer(&self) -> &[LogQueueEntry] {
        &self.records
    }

    pub fn entries(&self) -> impl Iterator<Item = (log::Level, &str)> {
        let record_count = self.records_offset.min(self.records.len());
        self.records[0..record_count].iter().filter_map(|entry| {
            let message = unsafe {
                /*
                 * The message must be valid utf-8 as it has been previously passed in as str.
                 */
                core::str::from_utf8_unchecked(
                    &self.buffer[entry.message_offset..entry.message_offset + entry.message_length],
                )
            };
            Some((entry.level, message))
        })
    }

    pub fn clear_queue(&mut self) -> (usize, usize) {
        let records_overflow_count = self.records_overflow_count;
        let buffer_overflow_count = self.buffer_overflow_count;

        self.records_offset = 0;
        self.records_overflow_count = 0;

        self.buffer_offset = 0;
        self.buffer_overflow_count = 0;

        (records_overflow_count, buffer_overflow_count)
    }

    pub fn enqueue_entry(&mut self, level: log::Level, args: fmt::Arguments<'_>) {
        let entry_index = self.records_offset;
        if entry_index >= self.records.len() {
            /* No more records available */
            self.records_overflow_count += 1;
            return;
        }

        let mut buf_writer = BufferFmt::new(&mut self.buffer[self.buffer_offset..]);
        if fmt::write(&mut buf_writer, args).is_err() {
            if buf_writer.had_buffer_overflow() {
                /* Buffer overflowed. Increment overflow counter. */
                self.buffer_overflow_count += 1;
            } else {
                /* Format error. Silently drop entry. */
            }
            return;
        }

        let record = &mut self.records[entry_index];
        record.level = level;
        record.message_offset = self.buffer_offset;
        record.message_length = buf_writer.bytes_written();

        self.buffer_offset += buf_writer.bytes_written();
        self.records_offset += 1;
    }
}
