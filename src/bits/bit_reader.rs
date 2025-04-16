use std::fmt;
use std::fmt::Debug;
use std::io;

use crate::base::DebugLog;
use crate::bits::bit_ops::*;

// If true, print debug information.
const DEBUG: bool = true;

// Debug log.
const LOG: DebugLog = DebugLog::new("BitReader");

// Buffer size.
const BUF_SIZE: usize = 8 * 1024;

/// Read a bit stream from a byte source.
///
/// This implementation allows efficient peeking and consuming of bits.
pub struct BitReader<'a> {
    // The current data buffer. Stores the next bits aligned to msb.
    data: u64,

    // Number of bits available in `data`.
    bits_avail: u32,

    // Internal buffer.
    buf: Box<[u8; BUF_SIZE]>,

    // Position in the buffer.
    buf_pos: usize,

    // End of the buffer.
    buf_end: usize,

    // Reader.
    reader: &'a mut dyn io::Read,

    // Number of bytes read.
    bytes_read: usize,

    // Number of read errors that have occurred.
    num_read_errors: usize,
}

impl<'a> BitReader<'a> {
    /// Create a new instance.
    pub fn new(reader: &'a mut dyn io::Read) -> BitReader<'a> {
        BitReader {
            data: 0,
            bits_avail: 0,
            buf: Box::new([0; BUF_SIZE]),
            buf_pos: 0,
            buf_end: 0,
            reader,
            bytes_read: 0,
            num_read_errors: 0,
        }
    }

    /// Fill the data buffer with more bits so that more bits will be available via `peek()`.
    pub fn fill_data(&mut self) {
        if DEBUG {
            LOG.print(&format!("Fill data. bits_avail = {}", self.bits_avail));
        }
        let num_bytes = (64 - self.bits_avail) / 8;
        let data = self.next_bytes(num_bytes as usize);

        self.data |= shift_right(data, self.bits_avail);
        self.bits_avail += num_bytes * 8;
    }

    /// Peek at the current data buffer.
    ///
    /// The next bits to be read are msb-aligned. `bits_avail()` number of bits are available.
    #[inline]
    pub fn peek(&self) -> u64 {
        self.data
    }

    /// Returns number of bits in the `data()` buffer.
    #[inline]
    pub fn bits_avail(&self) -> u32 {
        self.bits_avail
    }

    /// Consume the next `bits` number of bits.
    /// This assumes that `bits`` <= `bits_avail()`.
    pub fn consume(&mut self, bits: u32) {
        self.data = shift_left(self.data, bits);
        self.bits_avail -= bits;
    }

    /// Read the next `bits` number of bits.
    ///
    /// Returned value is lsb-aligned.
    pub fn read_bits(&mut self, bits: u32) -> u64 {
        let result = shift_right(self.data, 64 - bits);

        if self.bits_avail >= bits {
            self.data = shift_left(self.data, bits);
            self.bits_avail -= bits;
            return result;
        }

        // Not enough bits, read the next 64 bits.
        let next = self.next_u64();
        let extra_bits = bits - self.bits_avail;
        self.bits_avail = 64 - extra_bits;

        self.data = shift_left(next, extra_bits);
        result | shift_right(next, self.bits_avail)
    }

    /// Finish the reader and return number of bytes read.
    pub fn finish(&mut self) -> usize {
        if DEBUG {
            LOG.print("Finish");
        }
        self.bytes_read -= self.buf_end - self.buf_pos;
        self.buf_pos = 0;
        self.buf_end = 0;
        self.bytes_read
    }

    pub fn num_read_errors(&self) -> usize {
        self.num_read_errors
    }

    // Reads the next 64-bit value.
    fn next_u64(&mut self) -> u64 {
        let pos = self.buf_pos;
        self.buf_pos += 8;

        // Fast path: we have enough data in the buffer.
        if self.buf_pos <= self.buf_end {
            return u64::from_be_bytes(self.buf[pos..self.buf_pos].try_into().unwrap());
        }

        // Slow path: we need data from the reader.
        let mut data: [u8; 8] = [0; 8];
        let bytes = self.buf_end - pos;
        for i in 0..bytes {
            data[i] = self.buf[pos + i];
        }

        // Fill buffer and add remaining bytes.
        self.fill_buf();
        for i in bytes..8 {
            if self.buf_pos < self.buf_end {
                data[i] = self.buf[self.buf_pos];
                self.buf_pos += 1;
            }
        }
        if DEBUG {
            LOG.print(&format!("Read {:#x}", u64::from_be_bytes(data)));
        }
        u64::from_be_bytes(data)
    }

    // Reads the next `num_bytes` bytes.
    // Returns the data in big-endian format. The data may contain more than `num_bytes` bytes.
    fn next_bytes(&mut self, num_bytes: usize) -> u64 {
        if DEBUG {
            LOG.print(&format!("Next {} bytes", num_bytes));
        }

        // Fast path: we have >= 8 bytes available.
        if self.buf_end - self.buf_pos >= 8 {
            let bytes: &[u8; 8] = &self.buf[self.buf_pos..self.buf_pos + 8].try_into().unwrap();
            self.buf_pos += num_bytes;
            return u64::from_be_bytes(*bytes);
        }

        // Slow path: read 1 byte at a time.
        let mut data: [u8; 8] = [0; 8];
        for i in 0..num_bytes {
            if self.buf_pos == self.buf_end {
                self.fill_buf();

                // If it's end of stream, let it be padded with 0s.
                if self.buf_pos == self.buf_end {
                    break;
                }
            }
            data[i] = self.buf[self.buf_pos];
            self.buf_pos += 1;
        }
        u64::from_be_bytes(data)
    }

    // Fill the buffer with more data.
    fn fill_buf(&mut self) {
        assert!(self.buf_pos >= self.buf_end);
        self.buf_pos = 0;
        let result = self.reader.read(&mut self.buf[..]);
        match result {
            Ok(size) => {
                if DEBUG {
                    LOG.print(&format!("Read {} bytes", size));
                }
                self.buf_end = size;
                self.bytes_read += size;

                // Handle end of stream.
                if size == 0 && DEBUG {
                    LOG.print("End of input stream");
                }
            }
            Err(e) => {
                // Instead of just panicking, allow the reader to continue.
                // The client should check the read_errors() of this reader.
                self.buf_end = 0;
                self.num_read_errors += 1;
                LOG.print(&format!("Error: {}", e));

                // But panic if there are too many errors to avoid infinite loops.
                if self.num_read_errors > 100 {
                    panic!("Too many read errors");
                }
            }
        }
    }
}

impl Debug for BitReader<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BitReader")
            .field("data", &format!("{:#x}", &self.data))
            .field("bits_avail", &self.bits_avail)
            .field("buffered bytes", &(self.buf_end - self.buf_pos))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    // Can read 0 bits.
    fn test_0_bits() -> std::io::Result<()> {
        let buffer = vec![1, 2, 3];
        let mut reader: Cursor<Vec<u8>> = Cursor::new(buffer);
        let mut bit_reader = BitReader::new(&mut reader);
        assert_eq!(bit_reader.read_bits(0), 0);
        assert_eq!(bit_reader.num_read_errors(), 0);
        let bytes_read = bit_reader.finish();
        assert_eq!(bytes_read, 0);
        Ok(())
    }

    #[test]
    // Read bits.
    fn test_read_bits() -> std::io::Result<()> {
        let buffer: Vec<u8> = vec![1, 2, 3, 0xff, 0x81, 0x53, 0x78, 0x12, 0x25, 0xab];
        let mut reader = Cursor::new(buffer);
        let mut bit_reader = BitReader::new(&mut reader);

        assert_eq!(bit_reader.read_bits(8), 1);
        assert_eq!(bit_reader.read_bits(8), 2);
        assert_eq!(bit_reader.read_bits(8), 3);
        assert_eq!(bit_reader.read_bits(32), 0xff815378);
        assert_eq!(bit_reader.read_bits(32), 0x1225ab00);
        assert_eq!(bit_reader.num_read_errors(), 0);
        let bytes_read = bit_reader.finish();
        assert_eq!(bytes_read, 10);
        Ok(())
    }

    #[test]
    // Reading past end of stream returns 0 trailing bits.
    fn test_end_of_stream() -> std::io::Result<()> {
        let buffer: Vec<u8> = vec![1, 2, 3, 4];
        let mut reader = Cursor::new(buffer);
        let mut bit_reader = BitReader::new(&mut reader);

        assert_eq!(bit_reader.read_bits(64), 0x0102030400000000);
        assert_eq!(bit_reader.read_bits(64), 0);
        assert_eq!(bit_reader.num_read_errors(), 0);
        let bytes_read = bit_reader.finish();
        assert_eq!(bytes_read, 4);
        Ok(())
    }

    #[test]
    // Test peeking and consuming bits.
    fn test_peek() -> std::io::Result<()> {
        let buffer: Vec<u8> = vec![0x12, 0x34, 0x56, 0x78];
        let mut reader = Cursor::new(buffer);
        let mut bit_reader = BitReader::new(&mut reader);

        // Can peek 64 bits.
        bit_reader.fill_data();
        assert_eq!(bit_reader.peek(), 0x1234567800000000);
        assert_eq!(bit_reader.bits_avail(), 64);

        // Consume 1 bit. 63 bits left.
        bit_reader.consume(1);
        assert_eq!(bit_reader.peek(), 0x2468acf000000000);
        assert_eq!(bit_reader.bits_avail(), 63);

        // Read/peek past end of stream.
        assert_eq!(bit_reader.read_bits(64), 0x2468acf000000000);
        assert_eq!(bit_reader.peek(), 0);

        assert_eq!(bit_reader.num_read_errors(), 0);
        let bytes_read = bit_reader.finish();
        assert_eq!(bytes_read, 4);
        Ok(())
    }
}
