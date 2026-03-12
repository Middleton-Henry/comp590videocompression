use bitbit::{BitReader, reader::Bit};
use std::collections::{BinaryHeap};
use std::cmp::Ordering;
use std::io::Read;

/// Node structure matches encoder
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

/// Build Huffman tree from frequencies
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

pub struct Decoder {
    root: Node,
}

// Predefined tree for performance and memeory limitations
pub const STATIC_FREQS: [u64; 256] = {
    let mut freqs = [1u64; 256];
    let mut i = 0;
    while i < 256 {
        let dist = (i as i32 - 128).abs();
        freqs[i] = (256 - dist) as u64; 
        i += 1;
    }
    freqs
};

impl Decoder {
    pub fn new() -> Self {
        Self { root: build_tree(&STATIC_FREQS) }
    }

    pub fn decode<R: Read, B: Bit>(&mut self, input: &mut BitReader<R, B>) -> u8 {
        let mut current_node = &self.root;
        
        loop {
            if let Some(symbol) = current_node.symbol {
                return symbol;
            }
            
            let bit = input.read_bit().expect("Failed to read bit: Unexpected EOF");
            current_node = if bit {
                current_node.right.as_ref().expect("Invalid tree structure")
            } else {
                current_node.left.as_ref().expect("Invalid tree structure")
            };
        }
    }
}