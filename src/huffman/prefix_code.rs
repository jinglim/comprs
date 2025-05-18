use std::fmt;
use std::mem;

use crate::bits::{BitReader, BitWriter};

// Type of the symbols used in the prefix tree.
type SymbolType = u16;

// Type of the code. This sets the limit of the length of the codes.
type CodeType = u32;

// This implementation supports up to this number of bits.
const PREFIX_CODE_MAX_BITS: usize = 32;

/// Prefix codes for a set of symbols.
#[derive(Clone)]
pub struct PrefixCode {
    pub num_symbols: SymbolType,

    // lengths[i] = a vec of symbols that have code length = i.
    pub lengths: Vec<Vec<SymbolType>>,
}

impl PrefixCode {
    /// Creates a new instance with `num_symbols` and the symbols sorted into buckets by code length.
    pub fn new(num_symbols: SymbolType, lengths: Vec<Vec<SymbolType>>) -> Self {
        assert!(lengths.len() <= PREFIX_CODE_MAX_BITS);
        Self {
            num_symbols,
            lengths,
        }
    }

    /// Sets the maximum code length to `max_length`.
    /// This adjusts the code lengths of some leaves to ensure a full huffman tree.
    pub fn apply_max_length_limit(&mut self, max_length: usize) {
        if max_length >= self.lengths.len() - 1 {
            return;
        }

        // Count the extra weight due to moving longest symbols to `max_length`.
        let mut delta: usize = 0;
        for level in max_length + 1..self.lengths.len() {
            delta += self.weight_delta(self.lengths[level].len(), max_length, level);

            // Move the symbols to max_length.
            let symbols = mem::take(&mut self.lengths[level]);
            self.lengths[max_length].extend(symbols);
        }

        // Rebalance the prefix tree, moving some symbols down (i.e. increasing code length).
        let mut delta_to_adjust = delta;
        self.adjust(max_length - 1, max_length, 0, &mut delta_to_adjust);
        assert!(delta_to_adjust == 0);

        self.lengths.truncate(max_length + 1);
    }

    // Weight changes when moving `num` symbols at `higher` level of the tree to `lower`
    // level (i.e. shorter code to longer code).
    fn weight_delta(&self, num: usize, higher: usize, lower: usize) -> usize {
        let longest = self.lengths.len() - 1;
        (num << (longest - higher)) - (num << (longest - lower))
    }

    // Adjust the symbols at specified `level`.
    fn adjust(&mut self, level: usize, max_length: usize, total_adjust: usize, delta: &mut usize) {
        // Recursively go up the tree (decreasing code length) until the highest level where
        // adjustment is necessary.
        let num_symbols = self.lengths[level].len();

        // Find the maximum weight adjustment possible from bottom up to this level.
        let max_adjust = self.weight_delta(num_symbols, level, max_length);
        let new_total_adjust = total_adjust + max_adjust;
        if new_total_adjust < *delta {
            assert!(level > 0, "Not possible to apply specified length limit");

            // Need to recurse upwards.
            self.adjust(level - 1, max_length, new_total_adjust, delta);
        }

        // Make adjustments at this level by moving some symbols to (level + 1).
        if *delta > 0 {
            let adjustment = self.weight_delta(1, level, level + 1);
            while *delta > total_adjust && !self.lengths[level].is_empty() {
                let symbol = self.lengths[level].pop().unwrap();
                self.lengths[level + 1].push(symbol);
                *delta -= adjustment;
            }
        }
    }
}

impl PrefixCode {
    /// Generate codes for encoding.
    /// Returns a Vec of (64-bit code, bit length) for each symbol.
    pub fn generate_encoder_table(&self) -> Vec<(CodeType, u8)> {
        assert!(self.lengths.len() <= CodeType::BITS as usize);

        let mut codes: Vec<(CodeType, u8)> = vec![(0, 0); self.num_symbols as usize];
        let mut code: CodeType = 0;
        for i in 1..self.lengths.len() {
            if !self.lengths[i].is_empty() {
                for &symbol in self.lengths[i].iter() {
                    codes[symbol as usize] = (code, i as u8);
                    code += 1;
                }
            }
            code <<= 1;
        }
        codes
    }

    /// Encode (i.e. serialize) the code lengths table.
    /// This is a simple implementation, not optimized for minimizing compression size.
    pub fn encode_coding_table(&self, bit_writer: &mut BitWriter) {
        bit_writer.write_bits(self.num_symbols as u64, SymbolType::BITS);

        for i in 1..self.lengths.len() {
            let symbols = &self.lengths[i];
            if !symbols.is_empty() {
                // Code length.
                bit_writer.write_bits(i as u64, 32);

                // The symbols.
                bit_writer.write_bits(symbols.len() as u64, SymbolType::BITS);
                for &symbol in symbols.iter() {
                    bit_writer.write_bits(symbol as u64, SymbolType::BITS);
                }
            }
        }
        // Terminator.
        bit_writer.write_bits(0, 32);
    }
}

// Size of the decode lookup table.
const DECODE_TABLE_BITS: u32 = 6;

// Max size of the secondary decode lookup table.
const MAX_SECONDARY_TABLE_BITS: u32 = 4;

// Special symbol to indicate slow decode path.
const SLOW_DECODE_SYMBOL: SymbolType = SymbolType::MAX;

impl PrefixCode {
    /// Create a decoder.
    pub fn generate_decoder(&self) -> PrefixDecoder {
        let mut code_table: Vec<SymbolType> = Vec::with_capacity(1 << DECODE_TABLE_BITS);

        // Fill in the primary level decode table.
        for i in 1..(DECODE_TABLE_BITS + 1).min(self.lengths.len() as u32) {
            let symbols = &self.lengths[i as usize];
            if !symbols.is_empty() {
                let multiples = 1 << (DECODE_TABLE_BITS - i);
                for &symbol in symbols.iter() {
                    for _ in 0..multiples {
                        code_table.push(symbol);
                    }
                }
            }
        }

        let mut secondary_table_bits = 0;
        let mut slow_decode_table: Vec<SlowDecode> = Vec::new();

        // Build the secondary level decode table, if necessary.
        if self.lengths.len() as u32 > DECODE_TABLE_BITS {
            // Keep track of current position, and fill the rest of the entries temporarily.
            let mut pos = code_table.len();
            code_table.resize(1 << DECODE_TABLE_BITS, 0);

            // Size of the secondary table.
            secondary_table_bits =
                ((self.lengths.len() as u32) - 1 - DECODE_TABLE_BITS).min(MAX_SECONDARY_TABLE_BITS);

            // Current pos of the secondary table.
            let mut sec_pos = 0;
            let sec_table_mask = (1 << secondary_table_bits) - 1;

            for len in DECODE_TABLE_BITS + 1..DECODE_TABLE_BITS + secondary_table_bits + 1 {
                let symbols = &self.lengths[len as usize];
                if !symbols.is_empty() {
                    let multiples = 1 << (DECODE_TABLE_BITS + secondary_table_bits - len);

                    for &symbol in symbols.iter() {
                        if sec_pos & sec_table_mask == 0 {
                            // Set up the link from primary table to secondary table.
                            code_table[pos] = self.num_symbols + (code_table.len() as SymbolType);
                            pos += 1;
                            sec_pos = 0;
                        }
                        for _ in 0..multiples {
                            code_table.push(symbol);
                        }
                        sec_pos += multiples;
                    }
                }
            }

            // Set up slow path if needed.
            if self.lengths.len() as u32 > DECODE_TABLE_BITS + secondary_table_bits + 1 {
                // Fill remaining slots in secondary table.
                if sec_pos > 0 {
                    code_table.resize(
                        code_table.len() + (1 << secondary_table_bits) - sec_pos,
                        SLOW_DECODE_SYMBOL,
                    );
                }

                // Fill the rest of the primary entries if necessary.
                if pos < (1 << DECODE_TABLE_BITS) {
                    while pos < (1 << DECODE_TABLE_BITS) {
                        code_table[pos] = self.num_symbols + (code_table.len() as SymbolType);
                        pos += 1;
                    }
                    code_table.resize(
                        code_table.len() + (1 << secondary_table_bits),
                        SLOW_DECODE_SYMBOL,
                    );
                }

                // Create slow decode table for longer codes.
                let mut code: u64 = 0;
                for i in 1..self.lengths.len() {
                    let len = self.lengths[i].len();
                    if len > 0 && i > (DECODE_TABLE_BITS + secondary_table_bits) as usize {
                        slow_decode_table.push(SlowDecode {
                            length: i as u32,
                            symbols: self.lengths[i].clone(),
                            base: code,
                        });
                    }
                    code = (code + len as u64) << 1;
                }
            }
        }

        PrefixDecoder::new(
            self.num_symbols,
            secondary_table_bits,
            code_table,
            self.code_lengths(),
            slow_decode_table,
        )
    }

    // Decode (i.e. deserialize) the code lengths table and create a PrefixCode instance.
    pub fn decode_coding_table(bit_reader: &mut BitReader) -> Result<Self, &'static str> {
        const ERROR_STR: &str = "Decode error";
        let mut lengths: Vec<Vec<SymbolType>> = vec![Vec::new()];
        let num_symbols = bit_reader.read_bits(SymbolType::BITS) as SymbolType;
        loop {
            let len = bit_reader.read_bits(32) as usize;
            if len == 0 {
                break;
            }
            if len < lengths.len() || len > PREFIX_CODE_MAX_BITS {
                return Err(ERROR_STR);
            }
            while len > lengths.len() {
                lengths.push(Vec::new());
            }

            // Read the number of symbols and then the symbols.
            let num = bit_reader.read_bits(SymbolType::BITS) as SymbolType;
            let mut symbols: Vec<SymbolType> = Vec::new();
            for _ in 0..num {
                symbols.push(bit_reader.read_bits(SymbolType::BITS) as SymbolType);
            }
            lengths.push(symbols);
        }
        if bit_reader.num_read_errors() > 0 {
            return Err(ERROR_STR);
        }
        Ok(Self {
            num_symbols,
            lengths,
        })
    }

    /// Creates a table of code length of each symbol.
    pub fn code_lengths(&self) -> Vec<u8> {
        let mut code_lengths: Vec<u8> = vec![0; self.num_symbols as usize];
        for i in 0..self.lengths.len() {
            for &symbol in self.lengths[i].iter() {
                code_lengths[symbol as usize] = i as u8;
            }
        }
        code_lengths
    }
}

impl fmt::Display for PrefixCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.lengths)
    }
}

struct SlowDecode {
    length: u32,
    base: u64,
    symbols: Vec<SymbolType>,
}

/// Decoder for PrefixCode.
pub struct PrefixDecoder {
    num_symbols: SymbolType,
    secondary_table_bits: u32,
    code_table: Vec<u16>,
    code_lengths: Vec<u8>,
    slow_decode_table: Vec<SlowDecode>,
}

impl PrefixDecoder {
    fn new(
        num_symbols: SymbolType,
        secondary_table_bits: u32,
        code_table: Vec<SymbolType>,
        code_lengths: Vec<u8>,
        slow_decode_table: Vec<SlowDecode>,
    ) -> Self {
        Self {
            num_symbols,
            secondary_table_bits,
            code_table,
            code_lengths,
            slow_decode_table,
        }
    }

    /// Decodes a symbol.
    pub fn decode(&self, bit_reader: &mut BitReader) -> SymbolType {
        // Must have this number of bits available to decode.
        if bit_reader.bits_avail() < PREFIX_CODE_MAX_BITS as u32 {
            bit_reader.fill_data();
        }
        let peek_data: u64 = bit_reader.peek();

        // Primary lookup.
        let mut symbol = self.code_table[(peek_data >> (64 - DECODE_TABLE_BITS)) as usize];
        if symbol < self.num_symbols {
            bit_reader.consume(self.code_lengths[symbol as usize] as u32);
            return symbol;
        }

        // Look up secondary table.
        let secondary_index =
            ((peek_data << DECODE_TABLE_BITS) >> (64 - self.secondary_table_bits)) as usize;
        symbol = self.code_table[(symbol - self.num_symbols) as usize + secondary_index];
        if symbol < self.num_symbols {
            bit_reader.consume(self.code_lengths[symbol as usize] as u32);
            return symbol;
        }

        // Slow path.
        for decode in self.slow_decode_table.iter() {
            let shifted_data = peek_data >> (64 - decode.length);
            let delta = (shifted_data - decode.base) as usize;
            if delta < decode.symbols.len() {
                symbol = decode.symbols[delta];
                bit_reader.consume(decode.length);
                return symbol;
            }
        }
        panic!("This shouldn't happen");
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::bits::{BitReader, BitWriter};
    use std::collections::HashSet;
    use std::io;

    // Check that the code lengths are properly assigned such that the huffman tree is full.
    pub fn validate_prefix_code(code_lengths: &PrefixCode) {
        assert_eq!(code_lengths.lengths[0].len(), 0);
        let lengths = &code_lengths.lengths;
        let mut sum: u64 = 0;
        let mut weight = 1u64 << 62;
        let mut seen: HashSet<SymbolType> = HashSet::new();
        let mut num_symbols = 0;
        for i in 1..lengths.len() {
            sum += lengths[i].len() as u64 * weight;
            weight = weight >> 1;
            num_symbols += lengths[i].len();

            // Check that the symbols are unique.
            for symbol in lengths[i].iter() {
                assert!(!seen.contains(symbol));
                seen.insert(*symbol);
            }
        }
        if num_symbols == 1 {
            // Special case for a single symbol.
            assert_eq!(sum, 1 << 62);
        } else {
            assert_eq!(sum, 1 << 63);
        }
    }

    #[test]
    fn test_apply_max_length_limit() {
        fn test(code_lengths: &mut PrefixCode, max_lengths: &[usize]) {
            for max_length in max_lengths {
                let mut copy = code_lengths.clone();
                validate_prefix_code(&copy);
                copy.apply_max_length_limit(*max_length);
                assert!(copy.lengths.len() <= *max_length + 1);
                validate_prefix_code(&copy);
            }
        }

        test(
            &mut PrefixCode::new(4, vec![vec![], vec![0], vec![1], vec![2, 3]]),
            &[2, 3, 4],
        );

        test(
            &mut PrefixCode::new(
                6,
                vec![vec![], vec![0], vec![1], vec![2], vec![3], vec![4, 5]],
            ),
            &[3, 4, 5],
        );

        test(
            &mut PrefixCode::new(6, vec![vec![], vec![0], vec![1], vec![], vec![2, 3, 4, 5]]),
            &[3, 4],
        );

        test(
            &mut PrefixCode::new(
                26,
                vec![
                    vec![],
                    vec![0],
                    vec![],
                    vec![1],
                    vec![],
                    vec![],
                    vec![
                        2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                        23, 24, 25,
                    ],
                ],
            ),
            &[5, 6],
        );

        test(
            &mut PrefixCode::new(
                7,
                vec![
                    vec![],
                    vec![0],
                    vec![1],
                    vec![2],
                    vec![3],
                    vec![4],
                    vec![5, 6],
                ],
            ),
            &[3],
        );

        test(
            &mut PrefixCode::new(
                34,
                vec![
                    vec![],
                    vec![0],
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    (1..33).collect::<Vec<SymbolType>>(),
                ],
            ),
            &[6],
        );
    }

    #[test]
    fn test_generate_encoder_table() {
        fn test(prefix_code: &PrefixCode, expected: &Vec<(CodeType, u8)>) {
            validate_prefix_code(&prefix_code);
            let codes = prefix_code.generate_encoder_table();
            for i in 0..prefix_code.num_symbols {
                assert_eq!(codes[i as usize], expected[i as usize]);
            }
        }

        test(&PrefixCode::new(1, vec![vec![], vec![0]]), &vec![(0b00, 1)]);

        test(
            &PrefixCode::new(4, vec![vec![], vec![], vec![0, 1, 2, 3]]),
            &vec![(0b00, 2), (0b01, 2), (0b10, 2), (0b11, 2)],
        );

        test(
            &PrefixCode::new(3, vec![vec![], vec![0], vec![1, 2]]),
            &vec![(0b0, 1), (0b10, 2), (0b11, 2)],
        );

        test(
            &PrefixCode::new(6, vec![vec![], vec![0], vec![1], vec![], vec![2, 3, 4, 5]]),
            &vec![
                (0b0, 1),
                (0b10, 2),
                (0b1100, 4),
                (0b1101, 4),
                (0b1110, 4),
                (0b1111, 4),
            ],
        );
    }

    fn create_prefix_table(data: &[SymbolType]) -> PrefixCode {
        let mut symbol: SymbolType = 0;
        let mut lengths: Vec<Vec<SymbolType>> = Vec::new();
        for &num_symbols in data.iter() {
            lengths.push((symbol..symbol + num_symbols).into_iter().collect());
            symbol += num_symbols;
        }
        PrefixCode::new(symbol, lengths)
    }

    #[test]
    fn test_generate_decoder() {
        fn test(prefix_code: &PrefixCode) {
            validate_prefix_code(&prefix_code);
            prefix_code.generate_decoder();
        }

        test(&PrefixCode::new(
            6,
            vec![vec![], vec![0], vec![1], vec![], vec![2, 3, 4, 5]],
        ));

        test(&create_prefix_table(&[
            0, 0, 0, 2, 6, 4, 12, 4, 1, 5, 10, 11, 7, 2, 4, 4, 5, 3, 2, 5, 4, 1, 4, 4,
        ]));
    }

    #[test]
    fn test_encode_decode_prefix_code() {
        fn test(prefix_code: &PrefixCode) {
            validate_prefix_code(&prefix_code);
            let mut encode_cursor = io::Cursor::new(Vec::new());
            let mut writer = BitWriter::new(&mut encode_cursor);
            prefix_code.encode_coding_table(&mut writer);
            writer.finish();

            let mut decode_cursor = io::Cursor::new(encode_cursor.into_inner());
            let mut reader = BitReader::new(&mut decode_cursor);
            let decoded_prefix_code = PrefixCode::decode_coding_table(&mut reader).unwrap();

            assert_eq!(prefix_code.num_symbols, decoded_prefix_code.num_symbols);
            assert_eq!(prefix_code.lengths, decoded_prefix_code.lengths);
        }

        test(&create_prefix_table(&[
            0, 0, 0, 2, 6, 4, 12, 4, 1, 5, 10, 11, 7, 2, 4, 4, 5, 3, 2, 5, 4, 1, 4, 4,
        ]));
    }

    #[test]
    fn test_encode_decode() {
        fn test(prefix_code: &PrefixCode, input: Vec<SymbolType>) {
            // Encode
            let mut encode_cursor = io::Cursor::new(Vec::new());
            let mut writer = BitWriter::new(&mut encode_cursor);

            let encoder_table = prefix_code.generate_encoder_table();
            for &symbol in input.iter() {
                let (code, len) = encoder_table[symbol as usize];
                writer.write_bits(code as u64, len as u32);
            }
            writer.finish();
            assert_eq!(writer.num_write_errors(), 0);

            // Decode
            let mut decode_cursor = io::Cursor::new(encode_cursor.into_inner());
            let mut reader = BitReader::new(&mut decode_cursor);

            let decoder = prefix_code.generate_decoder();
            for i in 0..input.len() {
                let symbol = decoder.decode(&mut reader);
                assert_eq!(symbol, input[i]);
            }
        }

        test(
            &PrefixCode::new(
                11,
                vec![
                    vec![],
                    vec![0],
                    vec![1],
                    vec![],
                    vec![2, 3, 4],
                    vec![5],
                    vec![6],
                    vec![7],
                    vec![8],
                    vec![9, 10],
                ],
            ),
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        );
    }
}
