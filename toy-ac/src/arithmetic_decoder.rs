use super::range::Range;
use super::symbol_model::SymbolModel;
use bitbit::BitReader;
use bitbit::reader::Bit;
use std::io::Read;

#[derive(Debug)]
pub struct Decoder {
    range: Range,
    buffer: u32,
    initialized: bool,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            range: Range::new(32),
            buffer: 0x0,
            initialized: false,
        }
    }

    pub fn decode<'a, T: Eq, R: Read, B: Bit>(
        &mut self,
        m: &'a dyn SymbolModel<T>,
        input: &mut BitReader<R, B>,
    ) -> &'a T {
        // Load bits if first time
        if !self.initialized {
            match input.read_bits(32) {
                Ok(bits) => self.buffer = bits,
                Err(_) => panic!("Must have at least 32 bits to read initially."),
            }
            self.initialized = true;
        }
        // Bits in decoding buffer should always be in range [low, high]
        assert!(self.buffer as u64 >= self.range.low());
        assert!(self.buffer as u64 <= self.range.high());
        


        let range_width = self.range.width();
        let low = self.range.low();
        let total = m.total() as u64;
        let offset = self.buffer as u64 - low;

        let v = (((offset+1)* total -1) / range_width) as u32;
        let (result, int_start, int_end) = m.lookup(v);

        let new_low = low + (range_width * int_start as u64) / total;
        let new_high = low + (range_width * int_end as u64) / total  - 1;

        self.range.reduce(new_high, new_low);
        while self.range.hob_match() {
            let is_one = self.range.shift_hob();
            assert!(is_one == (self.buffer & 0x80000000 != 0));

            match input.read_bit() {
                Ok(bit) => self.buffer = self.buffer << 1 | if bit { 0x1 } else { 0x0 },
                Err(_) => panic!("Error reading bit"),
            }
        }

        assert!(!self.range.hob_match());

        while self.range.in_middle() {
            self.range.shift_sob();
            let buffer_hob_is_one = (self.buffer & 0x80000000) != 0;
            match input.read_bit() {
                Ok(bit) => {
                    self.buffer = self.buffer << 1 | if bit { 0x1 } else { 0x0 };
                    if buffer_hob_is_one {
                        self.buffer |= 0x80000000
                    } else {
                        self.buffer &= !0x80000000
                    }
                }
                Err(_) => panic!("Erorr reading bit"),
            }
        }
        return result;
    }

    pub fn high(&self) -> u64 {
        self.range.high()
    }

    pub fn low(&self) -> u64 {
        self.range.low()
    }

    pub fn buffer(&self) -> u32 {
        self.buffer
    }
}
