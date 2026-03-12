# Custom Video Commpression Program
## Overview
1) Implmented user interface for choosing video compression options
2) Implemented Huffman coding
3) Implemented Discrete Cosine Transform with variable Quantization

This program relies on rustdct for the DCT which I imported separately:

 $ cargo add rustdct --package assgn1

## Huffman Encoding
The huffman encoder works by taking a byte (0-255) and converting it into a variable length bit code such that more frequent symbols have shorter codes and vice-versa. My original approach to this encoder used an adaptive frequency builder, but this approach led to extremely long performance times (I did not time it but is was roughly 150x longer than current version), so I swapped to a static version with pre-cached frequency values. This approach while much faster, probably reduces the effectiveness of the algorithm. As you'll see in the tests, this approach undermines the point of using huffman entirely.

## Discrete Cosine Transform + Quantization
Placeholder Description
