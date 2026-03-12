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
The Discrete Cosine Transform translates a raster image (or any 2d array) into a series of sinusoidal coeffecients that can be combined back into the original image. To facilitate this process, I first break down the frame into 8x8 chunks which are fed into rustdct. This process returns a series of coeffeficients which can then be quantized for lossy compression. I used a jpeg quantization matrix as provided below, but I am not familiar with how the matrix was created or how it can be adjusted to better match video compression. Generally, smaller frequencies should be more heavily quantized. I also provided a user input such that 50 is standard jpeg quantization, 100 doubles jpeg quatization, and 1 is no compression.

Source for quantization matrix
https://stackoverflow.com/questions/29215879/how-can-i-generalize-the-quantization-matrix-in-jpeg-compression 

## Tests
The following results were performed on the provided bourne.mp4 video
### Arithmetic Encoding:
No Compression (1 quantization) – 17.66 compression ratio

Light Compression (25 quantization) – 11.14 compression ratio

Jpeg Compression (50 quantization) – 9.2 compression ratio

Heavy Compression (75 quantization) – 7.43 compression ratio

Maximum Compression (100 quantization) – 2.3 compression ratio

### Huffman Encoding:
No Compression (1 quantization) – 17.66 compression ratio

Light Compression (25 quantization) – 11.14 compression ratio

Jpeg Compression (50 quantization) – 9.2 compression ratio

Heavy Compression (75 quantization) – 7.43 compression ratio

Maximum Compression (100 quantization) – 2.3 compression ratio

### Analysis
Noteicably, Arithmetic and Huffman have identical compression ratios. This is probabaly a result of how I used a pre-cached static Huffman frequency array instead of an adaptive one which led to identical results.
