mod base;
mod bits;
mod coding;
mod huffman;

use crate::coding::{CompressionMethod, Tester};

fn main() {
    let tester = Tester::new();
    tester.run(vec![
        CompressionMethod::DynamicHuffmanCoding,
        CompressionMethod::StaticHuffmanCoding,
    ]);
}
