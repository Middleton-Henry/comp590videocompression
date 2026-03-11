use bitbit::{BitReader, reader::Bit};
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use std::io::Read;

/// Node structure matches encoder
#[derive(Debug)]
struct Node {
    symbol: Option<u8>,
    freq: u64,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        other.freq.cmp(&self.freq) // min-heap order
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.freq == other.freq
    }
}

impl Eq for Node {}

/// Build Huffman tree from frequencies
fn build_tree(freqs: &[u64; 256]) -> Node {
    let mut heap = BinaryHeap::new();

    for (sym, &freq) in freqs.iter().enumerate() {
        if freq > 0 {
            heap.push(Node {
                symbol: Some(sym as u8),
                freq,
                left: None,
                right: None,
            });
        }
    }

    while heap.len() > 1 {
        let a = heap.pop().unwrap();
        let b = heap.pop().unwrap();
        heap.push(Node {
            symbol: None,
            freq: a.freq + b.freq,
            left: Some(Box::new(a)),
            right: Some(Box::new(b)),
        });
    }

    heap.pop().unwrap()
}

/// Adaptive Huffman decoder
pub struct Decoder {
    freqs: [u64; 256],
    tree: Node,
}

impl Decoder {
    /// Start decoder with uniform initial frequencies
    pub fn new() -> Self {
        let freqs = [1u64; 256];
        let tree = build_tree(&freqs);
        Self { freqs, tree }
    }

    /// Decode one symbol from bitstream
    pub fn decode<R: Read, B: Bit>(&mut self, input: &mut BitReader<R, B>) -> u8 {
        let mut node = &self.tree;

        loop {
            match (node.symbol, &node.left, &node.right) {
                (Some(symbol), _, _) => {
                    // Update frequency after decoding
                    self.freqs[symbol as usize] += 1;
                    // Rebuild tree and replace old one
                    self.tree = build_tree(&self.freqs);
                    return symbol;
                }
                (None, Some(left), Some(right)) => {
                    let bit = input.read_bit().expect("Error reading bit");
                    node = if bit { right } else { left };
                }
                _ => panic!("Invalid Huffman node"),
            }
        }
    }
}