use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use bitbit::BitWriter;
use std::io::Write;
use std::error::Error;


#[derive(Debug)]

//Node strecture for building tree
struct Node {
    symbol: Option<u8>,
    freq: u64,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        // invert for minheap 
        other.freq.cmp(&self.freq)
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


// Build tree from frequency table
fn build_tree(freqs: &[u64; 256]) -> Node {
    let mut heap = BinaryHeap::new();
    for (sym, &freq) in freqs.iter().enumerate() {
        if freq > 0 {
            heap.push(Node { symbol: Some(sym as u8), freq, left: None, right: None });
        }
    }
    while heap.len() > 1 {
        let a = heap.pop().unwrap();
        let b = heap.pop().unwrap();
        heap.push(Node { symbol: None, freq: a.freq + b.freq, left: Some(Box::new(a)), right: Some(Box::new(b)) });
    }
    heap.pop().unwrap()
}

// Build lookup table for encoder; depth first
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







//keep same interface as arithmetic encoder for ease of swapping in main code
pub struct Encoder {
    freqs: [u64; 256],
    codes: HashMap<u8, Vec<bool>>,
    bits_written: u64,
}

impl Encoder {
    pub fn new() -> Self {
        let freqs = [1u64; 256]; // uniform initial frequencies
        let tree = build_tree(&freqs);
        let mut codes = HashMap::new();
        build_codes(&tree, Vec::new(), &mut codes);
        Self { freqs, codes, bits_written: 0 }
    }

    //per frame
    pub fn encode<W: std::io::Write>(&mut self, frame: &[u8], writer: &mut BitWriter<W>) {
        // update frequency counts for the frame
        for &symbol in frame {
            self.freqs[symbol as usize] += 1;
        }

        // rebuild Huffman tree and codes per frame
        let tree = build_tree(&self.freqs);
        self.codes.clear();
        build_codes(&tree, Vec::new(), &mut self.codes);

        // write all symbols in the frame
        for &symbol in frame {
            if let Some(code) = self.codes.get(&symbol) {
                for &bit in code {
                    writer.write_bit(bit).unwrap();
                    self.bits_written += 1;
                }
            }
        }
    }

    pub fn bits_written(&self) -> u64 {
        self.bits_written
    }

    pub fn finish<W: Write>(&mut self, _writer: &mut BitWriter<W>) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}