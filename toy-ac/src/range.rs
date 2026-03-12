#[derive(Debug)]
pub struct Range {
    bw: u32,
    high: u64,
    low: u64,
}

impl Range {
    pub fn new(buffer_width: u32) -> Self {
        if buffer_width > 63 || buffer_width < 2{
            panic!("Illegal range buffer width")
        }

        Self {
            bw: buffer_width,
            high: (!0x0) >> (64-buffer_width),
            low: 0x0,
        }
    }

    fn hob_mask(&self) -> u64 {0x1 << (self.bw-1)}
    fn range_mask(&self) -> u64 {0xffffffffffffffff >> (64-self.bw)}
    fn three_quarter_mark(&self) -> u64 {0x3 << (self.bw-2)}
    fn quarter_mark(&self) -> u64 {(!self.three_quarter_mark()) & self.range_mask()}

    pub fn width(&self) -> u64 {
        self.high - self.low + 1
    }

    pub fn low(&self) -> u64 {
        self.low
    }
    
    pub fn high(&self) -> u64 {
        self.high
    }
    
    pub fn reduce(&mut self, h: u64, l: u64) {
        if h > self.high || h < l || l < self.low {
            panic!("Illegal range reduction");
        }
        self.high = h;
        self.low = l;
    }

    pub fn hob_match(&self) -> bool {
        (self.high & self.hob_mask()) == (self.low & self.hob_mask())
    }

    pub fn shift_hob(&mut self) -> bool {
        if !self.hob_match() {
            panic!("High order bit should only be shifted if matched")
        }
        let is_bit_set = (self.high & self.hob_mask()) != 0;

        self.high = ((self.high << 1) & self.range_mask()) | 0x1;
        self.low = (self.low << 1) & self.range_mask();

        return is_bit_set;
    }

    pub fn in_middle(&self) -> bool {
        self.high < self.three_quarter_mark() && self.low > self.quarter_mark()
    }
    /*
    pub fn shift_sob(&mut self) {
        if !self.in_middle() {
            panic!("Second order bit should only be shifted if range is in middle")
        }

        self.high = ((self.high << 1) & self.range_mask()) | 0x1 | self.hob_mask();
        self.low = ((self.low << 1) & self.range_mask()) & (!self.hob_mask());
    }
    */
    pub fn shift_sob(&mut self) {
        let msb = self.high & 0x80000000;
        self.low = (self.low << 1) & 0x7FFFFFFF;
        self.high = ((self.high << 1) & 0x7FFFFFFF) | 1;
        self.low |= msb;
        self.high |= msb;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_init() {
        let range = Range::new(32);
        assert_eq!(range.high, 0xffffffff);
        assert_eq!(range.low, 0x0);
    }

    #[test]
    fn test_range_reduce_to_middle() {
        let mut range = Range::new(32);
        range.reduce(0x80000000, 0x7fffffff);
        assert!(!range.hob_match());
        assert!(range.in_middle());
        for _i in 0..30 {
            range.shift_sob();
            assert!(range.in_middle());
        }
        range.shift_sob();
        assert_eq!(range.high, 0xffffffff);
        assert_eq!(range.low, 0x0);
    }

    #[test]
    fn test_range_hob_match() {
        let mut range = Range::new(63);
        range.reduce(0x0123456789abcdef, 0x0123456789abcdee);
        assert_eq!(range.high, 0x0123456789abcdef);
        assert_eq!(range.low, 0x0123456789abcdee);
        assert!(range.hob_match());

        let mut shifted_bits = 0;
        while range.hob_match() {
            shifted_bits = shifted_bits << 1;
            if range.shift_hob() {
                shifted_bits += 1;
            }
        }
        assert_eq!(shifted_bits, 0x0123456789abcdef as u64 >> 1);
    }
}
