mod base;
mod bits;
mod coding;
mod huffman;

use crate::coding::{CompressionMethod, Tester};

fn main() {
    let tester = Tester::new(&[CompressionMethod::DynamicHuffmanCoding]);
    tester.run();
}
