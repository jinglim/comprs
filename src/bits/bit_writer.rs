use std::io;

use crate::base::DebugLog;
use crate::bits::bit_ops::*;

/// Number of bytes to buffer before flushing.
pub const BUF_SIZE: usize = 8 * 1024;

// If true, print debug information.
const DEBUG: bool = true;

// Debug log.
const LOG: DebugLog = DebugLog::new("BitWriter");

/// A bit stream writer that writes to a Writer.
pub struct BitWriter<'a> {
    // The current data buffer. Written data is msb aligned.
    data: u64,

    // Number of bits that can be written to `data`.
    bits_avail: u32,

    // Buffer to store the bytes to be written out to `writer`.
    buf: Vec<u8>,

    // External writer.
    writer: &'a mut dyn io::Write,

    // Total number of bytes written.
    bytes_written: usize,

    // Number of errors that occurred.
    write_errors: usize,
}

impl<'a> BitWriter<'a> {
    /// Create a new instance.
    pub fn new(writer: &'a mut dyn io::Write) -> Self {
        Self {
            data: 0,
            bits_avail: 64,
            buf: Vec::with_capacity(BUF_SIZE),
            writer,
            bytes_written: 0,
            write_errors: 0,
        }
    }

    /// Write `bits` number of bits from `data` (lsb aligned).
    pub fn write_bits(&mut self, data: u64, bits: u32) {
        if DEBUG {
            LOG.print(&format!("write_bits {:#x} {}", data, bits));
        }

        // Fast path: we have enough space in self.data.
        if self.bits_avail >= bits {
            self.data |= shift_left(data, self.bits_avail - bits);
            self.bits_avail -= bits;
            return;
        }

        // Write the bits that fit, and output the 64 bits in self.data.
        let remaining_bits = bits - self.bits_avail;
        let data_to_write = self.data | shift_right(data, remaining_bits);
        self.write_u64(data_to_write);

        // Move the remaining bits to self.data.
        let new_bits_avail = 64 - remaining_bits;
        self.data = shift_left(data, new_bits_avail);
        self.bits_avail = new_bits_avail;
    }

    // Flush the buffer to the writer.
    fn flush(&mut self) {
        if DEBUG {
            LOG.print("Flush");
        }
        let result = self.writer.write_all(&self.buf);
        if let Err(e) = result {
            // Allow the writer to continue, but keep track of errors.
            // Clients should check the number of write errors.
            LOG.print(&format!("Error writing to writer: {}", e));
            self.write_errors += 1;
        }
        self.bytes_written += self.buf.len();
        self.buf.clear();
    }

    /// Finish writing and return the total number of bytes written.
    /// Can only be called once.
    pub fn finish(&mut self) -> usize {
        if DEBUG {
            LOG.print("Finish");
        }

        // Flush the bits in self.data.
        let num_bytes = ((64 + 7 - self.bits_avail) / 8) as usize;
        if num_bytes > 0 {
            if DEBUG {
                LOG.print(&format!("Adding last {} bytes", num_bytes));
            }
            self.buf
                .extend_from_slice(&self.data.to_be_bytes()[..num_bytes]);
        }

        self.flush();
        self.bytes_written
    }

    /// Return the number of write errors encountered.
    pub fn num_write_errors(&self) -> usize {
        self.write_errors
    }

    // Write 8 bytes to the buffer. Flush the buffer if full.
    fn write_u64(&mut self, data: u64) {
        if DEBUG {
            LOG.print(&format!("write_u64 {:#x}", data));
        }
        self.buf.extend_from_slice(&data.to_be_bytes());
        if self.buf.len() >= BUF_SIZE {
            self.flush();
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1_bit() {
        let mut writer = io::Cursor::new(Vec::new());
        let mut bw = BitWriter::new(&mut writer);

        bw.write_bits(1, 1);
        let bytes_written = bw.finish();
        assert_eq!(bytes_written, 1);
        assert_eq!(bw.num_write_errors(), 0);
        assert_eq!(writer.into_inner(), vec![0x80]);
    }

    #[test]
    fn test_64_bits() -> std::io::Result<()> {
        let mut writer = io::Cursor::new(Vec::new());
        let mut bw = BitWriter::new(&mut writer);

        bw.write_bits(1, 8);
        bw.write_bits(0x1234567890AB, 48);
        bw.write_bits(1, 8);
        let bytes_written = bw.finish();
        assert_eq!(bytes_written, 8);
        assert_eq!(bw.num_write_errors(), 0);
        assert_eq!(
            writer.into_inner(),
            vec![1, 0x12, 0x34, 0x56, 0x78, 0x90, 0xAB, 1]
        );
        Ok(())
    }
}
