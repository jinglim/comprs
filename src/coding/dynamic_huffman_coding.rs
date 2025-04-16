use std::error::Error;
use std::io;

use crate::bits::{BitReader, BitWriter};
use crate::coding::decoder::{DecodeResult, Decoder};
use crate::coding::encoder::{EncodeResult, Encoder};
use crate::coding::input::InputSource;
use crate::coding::output::OutputSink;
use crate::huffman::DynamicHuffman;

// Symbol 256 = end of stream.
const NUM_SYMBOLS: u16 = 257;
const VALIDATE_TREE: bool = false;

// Input buffer size.
const READ_BUFFER_SIZE: usize = 8 * 1024;

pub struct DynamicHuffmanEncoder {
    huffman: DynamicHuffman,
}

impl DynamicHuffmanEncoder {
    pub fn new() -> Self {
        Self {
            huffman: DynamicHuffman::new(NUM_SYMBOLS),
        }
    }

    fn encode_loop(
        huffman: &mut DynamicHuffman,
        reader: &mut dyn io::Read,
        writer: &mut dyn io::Write,
    ) -> Result<EncodeResult, Box<dyn Error>> {
        let mut buffer: Box<[u8; READ_BUFFER_SIZE]> = Box::new([0; READ_BUFFER_SIZE]);
        let mut bit_writer = BitWriter::new(writer);
        let mut bytes_read = 0;
        loop {
            let len = reader.read(buffer.as_mut_slice())?;
            if len == 0 {
                break;
            }
            bytes_read += len;
            for &symbol in buffer[0..len].iter() {
                huffman.encode(symbol as u16, &mut bit_writer);
                if VALIDATE_TREE {
                    huffman.validate();
                }
            }
        }

        // Write the end of file marker.
        huffman.encode(256, &mut bit_writer);
        let bytes_written = bit_writer.finish();

        Ok(EncodeResult::new(bytes_read, bytes_written))
    }
}

impl Encoder for DynamicHuffmanEncoder {
    fn encode(
        &mut self,
        input: &mut InputSource,
        output: &mut OutputSink,
    ) -> Result<EncodeResult, Box<dyn Error>> {
        let mut reader = input.reader();
        let mut writer = output.writer();
        Self::encode_loop(&mut self.huffman, &mut reader, &mut writer)
    }
}

pub struct DynamicHuffmanDecoder {
    huffman: DynamicHuffman,
}

impl DynamicHuffmanDecoder {
    pub fn new() -> Self {
        Self {
            huffman: DynamicHuffman::new(NUM_SYMBOLS),
        }
    }

    fn decode_loop(
        huffman: &mut DynamicHuffman,
        reader: &mut dyn io::Read,
        writer: &mut dyn io::Write,
    ) -> Result<DecodeResult, Box<dyn Error>> {
        let mut buffer: Box<[u8; READ_BUFFER_SIZE]> = Box::new([0; READ_BUFFER_SIZE]);
        let mut bit_reader = BitReader::new(reader);
        let mut buffer_pos = 0;
        let mut bytes_written = 0;
        loop {
            let symbol = huffman.decode(&mut bit_reader);
            if symbol == 256 {
                break;
            }
            buffer[buffer_pos] = symbol as u8;
            buffer_pos += 1;
            if buffer_pos == READ_BUFFER_SIZE {
                writer.write_all(buffer.as_ref())?;
                buffer_pos = 0;
                bytes_written += READ_BUFFER_SIZE;
            }
        }
        let bytes_read = bit_reader.finish();
        writer.write_all(&buffer[0..buffer_pos])?;
        bytes_written += buffer_pos;

        Ok(DecodeResult::new(bytes_read, bytes_written))
    }
}

impl Decoder for DynamicHuffmanDecoder {
    fn decode(
        &mut self,
        input: &mut InputSource,
        output: &mut OutputSink,
    ) -> Result<DecodeResult, Box<dyn Error>> {
        let mut reader = input.reader();
        let mut writer = output.writer();
        Self::decode_loop(&mut self.huffman, &mut reader, &mut writer)
    }
}
