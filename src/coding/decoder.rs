use std::error::Error;
use std::fmt;

use crate::coding::input::InputSource;
use crate::coding::output::OutputSink;

pub struct DecodeResult {
    bytes_read: usize,
    bytes_written: usize,
}

impl DecodeResult {
    pub fn new(bytes_read: usize, bytes_written: usize) -> Self {
        Self {
            bytes_read,
            bytes_written,
        }
    }
}

impl fmt::Display for DecodeResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} bytes read, {} bytes written",
            self.bytes_read, self.bytes_written
        )
    }
}

pub trait Decoder {
    fn decode(
        &mut self,
        input: &mut InputSource,
        output: &mut OutputSink,
    ) -> Result<DecodeResult, Box<dyn Error>>;
}
