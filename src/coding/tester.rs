use std::error::Error;

use crate::coding::decoder::{DecodeResult, Decoder};
use crate::coding::dynamic_huffman_coding::{DynamicHuffmanDecoder, DynamicHuffmanEncoder};
use crate::coding::encoder::{EncodeResult, Encoder};
use crate::coding::input::InputSource;
use crate::coding::output::OutputSink;
use crate::coding::static_huffman_coding::{StaticHuffmanDecoder, StaticHuffmanEncoder};

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum CompressionMethod {
    DynamicHuffmanCoding,
    StaticHuffmanCoding,
}

type EncoderFactory = fn() -> Box<dyn Encoder>;
type DecoderFactory = fn() -> Box<dyn Decoder>;

// Dynamic huffman coding
fn create_dynamic_huffman_coding_encoder() -> Box<dyn Encoder> {
    Box::new(DynamicHuffmanEncoder::new())
}

fn create_dynamic_huffman_coding_decoder() -> Box<dyn Decoder> {
    Box::new(DynamicHuffmanDecoder::new())
}

fn create_static_huffman_coding_encoder() -> Box<dyn Encoder> {
    Box::new(StaticHuffmanEncoder::new())
}

fn create_static_huffman_coding_decoder() -> Box<dyn Decoder> {
    Box::new(StaticHuffmanDecoder::new())
}

struct CompressionFactory {
    name: String,
    method: CompressionMethod,
    encoder_factory: EncoderFactory,
    decoder_factory: DecoderFactory,
}

struct CompressionFactories {
    all: Vec<CompressionFactory>,
}

impl CompressionFactories {
    fn new() -> Self {
        let mut instance = Self { all: Vec::new() };
        instance.all.push(CompressionFactory {
            name: "DynamicHuffman".to_string(),
            method: CompressionMethod::DynamicHuffmanCoding,
            encoder_factory: || Box::new(DynamicHuffmanEncoder::new()),
            decoder_factory: || Box::new(DynamicHuffmanDecoder::new()),
        });
        instance.all.push(CompressionFactory {
            name: "StaticHuffman".to_string(),
            method: CompressionMethod::StaticHuffmanCoding,
            encoder_factory: || Box::new(StaticHuffmanEncoder::new()),
            decoder_factory: || Box::new(StaticHuffmanDecoder::new()),
        });
        instance
    }

    fn get_method(&self, method: CompressionMethod) -> &CompressionFactory {
        for factory in self.all.iter() {
            if factory.method == method {
                return factory;
            }
        }
        panic!();
    }
}

// For testing all coding methods.
pub struct Tester {
    factories: CompressionFactories,
}

impl Tester {
    pub fn new() -> Self {
        Self {
            factories: CompressionFactories::new(),
        }
    }

    /// Test encode a file.
    pub fn encode(&self, method: CompressionMethod) {
        let factory = self.factories.get_method(method);
        let mut encoder = (factory.encoder_factory)();

        let input_file = "/tmp/test";
        let mut input_data = InputSource::file(input_file);
        let mut output_data = OutputSink ::memory( Vec::new());
        println!("{} -> {}", input_data, output_data);
        let result = encoder.encode(&mut input_data, &mut output_data);
        self.report_encode_result(&result);
    }

    /// Run a series of tests using the methods.
    pub fn run(&self, methods: Vec<CompressionMethod>) {
        for &method in methods.iter() {
            let factory = self.factories.get_method(method);
            println!("{}:", factory.name);
            let mut encoder = (factory.encoder_factory)();
            let mut decoder = (factory.decoder_factory)();

            {
                // Create input data.
                let mut input_vec: Vec<u8> = Vec::new();
                for i in 0..1000 {
                    input_vec.push(((i % 32) + 32) as u8);
                }

                // Encode to memory
                let (result, input_vec, encoded_vec) =
                    self.encode_memory_to_memory(&mut encoder, input_vec, Vec::new());
                self.report_encode_result(&result);

                // Decode to memory.
                let (result, decoded_vec) =
                    self.decode_memory_to_memory(&mut decoder, encoded_vec, Vec::new());
                self.report_decode_result(&result);

                // Compare
                assert!(input_vec == decoded_vec);
            }
            println!();

            {
                // Encode and decode file to file.
                let input_file = "/tmp/test";
                let encoded_file = "/tmp/test.enc";
                let decoded_file = "/tmp/test.dec";

                let result = self.encode_file_to_file(&mut encoder, input_file, encoded_file);
                self.report_encode_result(&result);

                let result = self.decode_file_to_file(&mut decoder, encoded_file, decoded_file);
                self.report_decode_result(&result);

                // Compare
                let input_data = std::fs::read(input_file).unwrap();
                let decoded_data = std::fs::read(decoded_file).unwrap();
                assert!(input_data == decoded_data);
            }
            println!();
        }
    }

    fn encode_file_to_file(
        &self,
        encoder: &mut Box<dyn Encoder>,
        input_file: &str,
        output_file: &str,
    ) -> Result<EncodeResult, Box<dyn Error>> {
        let mut input_data = InputSource::file(input_file);
        let mut output_data = OutputSink::file(output_file);
        println!("{} -> {}", input_data, output_data);
        encoder.encode(&mut input_data, &mut output_data)
    }

    fn encode_memory_to_memory(
        &self,
        encoder: &mut Box<dyn Encoder>,
        input_vec: Vec<u8>,
        output_vec: Vec<u8>,
    ) -> (Result<EncodeResult, Box<dyn Error>>, Vec<u8>, Vec<u8>) {
        let mut input_data = InputSource::memory(input_vec);
        let mut output_data = OutputSink::memory(output_vec);
        println!("{} -> {}", input_data, output_data);
        let result = encoder.encode(&mut input_data, &mut output_data);
        (result, input_data.take_memory(), output_data.take_memory())
    }

    fn decode_memory_to_memory(
        &self,
        decoder: &mut Box<dyn Decoder>,
        input_vec: Vec<u8>,
        output_vec: Vec<u8>,
    ) -> (Result<DecodeResult, Box<dyn Error>>, Vec<u8>) {
        let mut input_data = InputSource::memory(input_vec);
        let mut output_data = OutputSink::memory(output_vec);
        println!("{} -> {}", input_data, output_data);
        let result = decoder.decode(&mut input_data, &mut output_data);
        (result, output_data.take_memory())
    }

    fn decode_file_to_file(
        &self,
        decoder: &mut Box<dyn Decoder>,
        input_file: &str,
        output_file: &str,
    ) -> Result<DecodeResult, Box<dyn Error>> {
        let mut input_data = InputSource::file(input_file);
        let mut output_data = OutputSink::file(output_file);
        println!("{} -> {}", input_data, output_data);
        decoder.decode(&mut input_data, &mut output_data)
    }

    fn report_encode_result(&self, result: &Result<EncodeResult, Box<dyn Error>>) {
        match result {
            Ok(result) => println!("  Encode result: {}", result),
            Err(e) => println!("  Error: {}", e),
        }
    }

    fn report_decode_result(&self, result: &Result<DecodeResult, Box<dyn Error>>) {
        match result {
            Ok(result) => println!("  Decode result: {}", result),
            Err(e) => println!("  Error: {}", e),
        }
    }
}
