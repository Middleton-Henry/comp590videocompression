use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use bitbit::BitWriter;
use std::io::Write;

#[derive(Debug)]
struct Node {
    symbol: Option<u8>,
    freq: u64,
    seq: u64,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        other.freq.cmp(&self.freq)
            .then(other.seq.cmp(&self.seq))  // ADD THIS LINE
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.freq == other.freq && self.symbol == other.symbol
    }
}

impl Eq for Node {}



fn build_tree(freqs: &[u64; 256]) -> Node {
    let mut heap = BinaryHeap::new();
    let mut counter = 0u64;  // ADD

    for (sym, &freq) in freqs.iter().enumerate() {
        if freq > 0 {
            heap.push(Node { symbol: Some(sym as u8), freq, seq: counter, left: None, right: None });
            counter += 1;  // ADD
        }
    }
    while heap.len() > 1 {
        let a = heap.pop().unwrap();
        let b = heap.pop().unwrap();
        heap.push(Node {
            symbol: None,
            freq: a.freq + b.freq,
            seq: counter,  // ADD
            left: Some(Box::new(a)),
            right: Some(Box::new(b)),
        });
        counter += 1;  // ADD
    }
    heap.pop().expect("Frequency table was empty")
}

fn build_codes(node: &Node, prefix: Vec<bool>, codes: &mut HashMap<u8, Vec<bool>>) {
    if let Some(sym) = node.symbol {
        codes.insert(sym, prefix);
        return;
    }
    if let Some(ref left) = node.left {
        let mut p = prefix.clone();
        p.push(false);
        build_codes(left, p, codes);
    }
    if let Some(ref right) = node.right {
        let mut p = prefix.clone();
        p.push(true);
        build_codes(right, p, codes);
    }
}

pub struct Encoder {
    bits_written: u64,
    codes: HashMap<u8, Vec<bool>>,
}

// Predefined tree for performance and memeory limitations
pub const STATIC_FREQS: [u64; 256] = {
    let mut freqs = [1u64; 256];
    let mut i = 0;
    while i < 256 {
        let dist = if i < 128 { 128 - i } else { i - 128 };
        freqs[i] = (256 - dist) as u64;
        i += 1;
    }
    freqs
};

impl Encoder {
    pub fn new() -> Self {
        let tree = build_tree(&STATIC_FREQS);
        let mut codes = HashMap::new();
        build_codes(&tree, Vec::new(), &mut codes);
        
        Self {
            bits_written: 0,
            codes,
        }
    }

    pub fn bits_written(&self) -> u64 {
        self.bits_written
    }

    pub fn encode_block<W: Write>(&mut self, data: &[u8], output: &mut BitWriter<W>) {
        
        for &byte in data {
            let code = self.codes.get(&byte).expect("Symbol not in Huffman tree");
            for &bit in code {
                output.write_bit(bit).unwrap();
                self.bits_written += 1;
            }
        }
    }

    pub fn finish<W: Write>(&mut self, _output: &mut BitWriter<W>) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}