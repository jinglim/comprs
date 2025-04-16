use std::error::Error;
use std::fmt;

use crate::coding::input::InputSource;
use crate::coding::output::OutputSink;

pub struct EncodeResult {
    bytes_read: usize,
    bytes_written: usize,
}

impl EncodeResult {
    pub fn new(bytes_read: usize, bytes_written: usize) -> Self {
        Self {
            bytes_read,
            bytes_written,
        }
    }
}

impl fmt::Display for EncodeResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Encode result: {} bytes read, {} bytes written",
            self.bytes_read, self.bytes_written
        )
    }
}

pub trait Encoder {
    fn encode(
        &mut self,
        input: &mut InputSource,
        output: &mut OutputSink,
    ) -> Result<EncodeResult, Box<dyn Error>>;
}
