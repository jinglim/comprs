use std::error::Error;
use std::io;

use crate::bits::{BitReader, BitWriter};
use crate::coding::decoder::{DecodeResult, Decoder};
use crate::coding::encoder::{EncodeResult, Encoder};
use crate::coding::input::InputSource;
use crate::coding::output::OutputSink;
use crate::huffman::{PrefixCode, StaticHuffman};

const NUM_SYMBOLS: u16 = 256;

// Input buffer size.
const READ_BUFFER_SIZE: usize = 8 * 1024;

pub struct StaticHuffmanEncoder {
    huffman: StaticHuffman,
}

impl StaticHuffmanEncoder {
    pub fn new() -> Self {
        Self {
            huffman: StaticHuffman::new(NUM_SYMBOLS),
        }
    }

    fn encode_loop(
        huffman: &mut StaticHuffman,
        input_length: u64,
        frequencies: Vec<u32>,
        reader: &mut dyn io::Read,
        writer: &mut dyn io::Write,
    ) -> Result<EncodeResult, Box<dyn Error>> {
        let mut prefix_code = huffman.build_from_weights(&frequencies);
        prefix_code.apply_max_length_limit(32);
        let encoder_table = prefix_code.generate_encoder_table();

        // Write out the input length.
        let mut bit_writer = BitWriter::new(writer);
        bit_writer.write_bits(input_length, 64);

        // Write the coding table.
        prefix_code.encode_coding_table(&mut bit_writer);

        let mut input_buf: Box<[u8; READ_BUFFER_SIZE]> = Box::new([0; READ_BUFFER_SIZE]);
        let mut bytes_read = 0;
        loop {
            let len = reader.read(input_buf.as_mut_slice())?;
            if len == 0 {
                break;
            }
            bytes_read += len;
            for &symbol in input_buf[0..len].iter() {
                let code = encoder_table[symbol as usize];
                bit_writer.write_bits(code.0 as u64, code.1 as u32);
            }
        }
        let bytes_written = bit_writer.finish();

        Ok(EncodeResult::new(bytes_read, bytes_written))
    }

}

impl Encoder for StaticHuffmanEncoder {
    fn encode(
        &mut self,
        input: &mut InputSource,
        output: &mut OutputSink,
    ) -> Result<EncodeResult, Box<dyn Error>> {
        let mut reader = input.reader();
        let mut writer = output.writer();

        let input_length = input.len();
        let frequencies = input.frequencies();
        Self::encode_loop(
            &mut self.huffman,
            input_length,
            frequencies,
            &mut reader,
            &mut writer,
        )
    }
}

pub struct StaticHuffmanDecoder {
    huffman: StaticHuffman,
}

impl StaticHuffmanDecoder {
    pub fn new() -> Self {
        Self {
            huffman: StaticHuffman::new(NUM_SYMBOLS),
        }
    }

    fn decode_loop(
        &self,
        reader: &mut dyn io::Read,
        writer: &mut dyn io::Write,
    ) -> Result<DecodeResult, Box<dyn Error>> {

        let mut bit_reader = BitReader::new(reader);
        let input_len = bit_reader.read_bits(64);

        let prefix_code = PrefixCode::decode_coding_table(&mut bit_reader)?;
        let decoder = prefix_code.generate_decoder();

        let mut buffer: Box<[u8; READ_BUFFER_SIZE]> = Box::new([0; READ_BUFFER_SIZE]);
        let mut bytes_written = 0;
        let mut buffer_pos = 0;
        for _ in 0..input_len as usize {
            let symbol = decoder.decode(&mut bit_reader);
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

impl Decoder for StaticHuffmanDecoder {
    fn decode(
        &mut self,
        input: &mut InputSource,
        output: &mut OutputSink,
    ) -> Result<DecodeResult, Box<dyn Error>> {
        let mut reader = input.reader();
        let mut writer = output.writer();
        self.decode_loop(&mut reader, &mut writer)
    }
}
