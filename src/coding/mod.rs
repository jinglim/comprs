mod decoder;
mod dynamic_huffman_coding;
mod encoder;
mod input;
mod output;
mod tester;

pub use dynamic_huffman_coding::DynamicHuffmanEncoder;
pub use tester::{CompressionMethod, Tester};
