/// A bitstream used mainly for development and debugging ANS compressed bitstreams.
pub struct DevReverseBitStream {
    // Store 1 bit in each u8.
    data: Vec<u8>,
}

impl DevReverseBitStream {
    pub fn for_writing() -> Self {
        Self { data: Vec::new() }
    }

    pub fn for_reading(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Return number of bits in the bit stream.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Write out 1 bit.
    pub fn write_1bit(&mut self, bit: u8) {
        assert!(bit == 0 || bit == 1);
        self.data.push(bit);
    }

    /// Read 1 bit.
    pub fn read_1bit(&mut self) -> u8 {
        self.data.pop().unwrap()
    }

    /// Write `num_bits` number of bits in `data` (aligned to lsb).
    pub fn write_bits(&mut self, data: u64, num_bits: u32) {
        for i in (0..num_bits).rev() {
            self.data.push(((data >> i) & 1) as u8);
        }
    }

    /// Read `num_bits` number of bits. Returns data aligned to lsb.
    pub fn read_bits(&mut self, num_bits: u32) -> u64 {
        let mut data: u64 = 0;
        for i in 0..num_bits {
            data |= (self.data.pop().unwrap() as u64) << i;
        }
        data
    }

    /// Remove all data from the bit stream.
    pub fn remove_data(&mut self) -> Vec<u8> {
        let mut new_data: Vec<u8> = Vec::new();
        std::mem::swap(&mut self.data, &mut new_data);
        new_data
    }

    pub fn print(&self) {
        println!("Bits: {:?}", String::from_utf8(self.data.clone()).unwrap());
    }
}

mod tests {
    use super::*;

    #[test]
    fn test_dev_bit_stream() {
        let mut writer = DevReverseBitStream::for_writing();
        writer.write_1bit(0);
        assert_eq!(writer.len(), 1);
        writer.write_1bit(1);
        writer.write_1bit(0);

        let mut reader = DevReverseBitStream::for_reading(writer.remove_data());
        assert_eq!(reader.len(), 3);
        assert_eq!(reader.read_1bit(), 0);
        assert_eq!(reader.len(), 2);
        assert_eq!(reader.read_1bit(), 1);
        assert_eq!(reader.len(), 1);
        assert_eq!(reader.read_1bit(), 0);
        assert_eq!(reader.len(), 0);
    }
}
