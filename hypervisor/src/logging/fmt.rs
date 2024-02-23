use core::fmt;

pub struct BufferFmt<'a> {
    buffer: &'a mut [u8],
    buffer_overflow: bool,
    bytes_written: usize,
}

impl<'a> BufferFmt<'a> {
    pub fn new(buffer: &'a mut [u8]) -> Self {
        Self {
            buffer,
            buffer_overflow: false,
            bytes_written: 0,
        }
    }

    pub fn bytes_written(&self) -> usize {
        self.bytes_written
    }

    pub fn had_buffer_overflow(&self) -> bool {
        self.buffer_overflow
    }
}

impl<'a> fmt::Write for BufferFmt<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let payload = s.as_bytes();
        if payload.len() + self.bytes_written > self.buffer.len() {
            self.buffer_overflow = true;
            return Err(fmt::Error);
        }

        self.buffer[self.bytes_written..self.bytes_written + payload.len()]
            .copy_from_slice(payload);
        self.bytes_written += payload.len();
        Ok(())
    }
}
