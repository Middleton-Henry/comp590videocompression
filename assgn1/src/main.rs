use std::env;
use std::path::PathBuf;

use ffmpeg_sidecar::command::FfmpegCommand;
use workspace_root::get_workspace_root;

use std::fs::File;
use std::io::BufReader;
use std::io::{BufWriter, Write};

use bitbit::BitReader;
use bitbit::BitWriter;
use bitbit::MSB;

use toy_ac::arithmetic_decoder::Decoder as ArithmeticDecoder;
use toy_ac::huffman_decoder::Decoder as HuffmanDecoder;
use toy_ac::arithmetic_encoder::Encoder as ArithmeticEncoder;
use toy_ac::huffman_encoder::Encoder as HuffmanEncoder;


// Two connected coding states
enum CodingType {
    ARITHMETIC(ArithmeticEncoder),
    HUFFMAN(HuffmanEncoder)
}

enum DecodingType {
    ARITHMETIC(ArithmeticDecoder),
    HUFFMAN(HuffmanDecoder)
}

use toy_ac::symbol_model::VectorCountSymbolModel;

use ffmpeg_sidecar::event::StreamTypeSpecificData::Video;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Make sure ffmpeg is installed
    ffmpeg_sidecar::download::auto_download().unwrap();

    // Command line options
    // -verbose, -no_verbose                Default: -no_verbose
    // -report, -no_report                  Default: -report
    // -check_decode, -no_check_decode      Default: -no_check_decode
    // -skip_count n                        Default: -skip_count 0
    // -count n                             Default: -count 10
    // -in file_path                        Default: bourne.mp4 in data subdirectory of workplace
    // -out file_path                       Default: out.dat in data subdirectory of workplace

    // Set up default values of options
    let mut verbose = false;
    let mut report = true;
    let mut check_decode = false;
    let mut skip_count = 0;
    let mut count = 10;

    let mut data_folder_path = get_workspace_root();
    data_folder_path.push("data");

    //selects the input video file
    //let mut input_file_path = data_folder_path.join("bourne.mp4");
    let mut input_file_path = select_input_file(&data_folder_path);

    let mut output_file_path = data_folder_path.join("out.dat");



    //prompt for encoding type
    println!("Select an encoding type:");
    println!("1. Arithmetic coding");
    println!("2. Huffman coding");

    let encoding_type : i32 = loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        //set value of encoding type based on user input or prompt again if invalid input
        match input.trim() {
            "1" => break 1,
            "2" => break 2,
            _ => println!("INVALID INPUT: please enter 1 or 2.")
        }
    };

    let mut enc = match encoding_type {
        1 => CodingType::ARITHMETIC(ArithmeticEncoder::new()),
        2 => CodingType::HUFFMAN(HuffmanEncoder::new()),
        _ => panic!("ERROR: Invalid encoding type detected")
    };


    println!("Enter quantization factor:");
    println!("(100 = highest compression, 50 = default jpeg compression, 1 = no compression)");
    let quant_factor: f32 = loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        match input.trim().parse::<f32>() {
            Ok(val) if val >= 1.0 && val <= 100.0 => break val,
            _ => println!("INVALID INPUT: Enter a number between 1 and 100."),
        }
    };

    let mut export_video = false;

    loop {
        println!("Export compressed video? (y/n)");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        match input.trim().to_lowercase().as_str() {
            "y" => {
                // Export the compressed file
                export_video = true;
                break;
            }
            "n" => {
                println!("Export cancelled.");
                break;
            }
            _ => {
                println!("Invalid input, please enter 'y' or 'n'.");
            }
        }
    }


    

    parse_args(
        &mut verbose,
        &mut report,
        &mut check_decode,
        &mut skip_count,
        &mut count,
        &mut input_file_path,
        &mut output_file_path,
    );

    // Run an FFmpeg command to decode video from inptu_file_path
    // Get output as grayscale (i.e., just the Y plane)

    let mut iter = FfmpegCommand::new() // <- Builder API like `std::process::Command`
        .input(input_file_path.to_str().unwrap())
        .format("rawvideo")
        .pix_fmt("gray8")
        .output("-")
        .spawn()? // <- Ordinary `std::process::Child`
        .iter()?; // <- Blocking iterator over logs and output

    // Figure out geometry of frame.
    let mut width = 0;
    let mut height = 0;

    let metadata = iter.collect_metadata()?;
    for i in 0..metadata.output_streams.len() {
        match &metadata.output_streams[i].type_specific_data {
            Video(vid_stream) => {
                width = vid_stream.width;
                height = vid_stream.height;

                if verbose {
                    println!(
                        "Found video stream at output stream index {} with dimensions {} x {}",
                        i, width, height
                    );
                }
                break;
            }
            _ => (),
        }
    }
    assert!(width != 0);
    assert!(height != 0);

    // Set up initial prior frame as uniform medium gray (y = 128)
    let mut prior_frame = vec![128 as u8; (width * height) as usize];
    

    let output_file = match File::create(&output_file_path) {
        Err(_) => panic!("Error opening output file"),
        Ok(f) => f,
    };

    // Setup bit writer and arithmetic encoder.

    let mut buf_writer = BufWriter::new(output_file);
    let mut bw = BitWriter::new(&mut buf_writer);

    //let mut enc = Encoder::new();

    // Set up arithmetic coding context(s)
    let pixel_difference_pdf = VectorCountSymbolModel::new((0..=255).collect());

    let quant_table = get_adjusted_quantization(quant_factor);
    let block_size = 8;
    let blocks_per_row = (width + block_size - 1) / block_size;
    let blocks_per_col = (height + block_size - 1) / block_size;

    // Process frames
    for frame in iter.filter_frames() {
        if frame.frame_num < skip_count {
            if verbose {
                println!("Skipping frame {}", frame.frame_num);
            }
        } else if frame.frame_num < skip_count + count {
            let current_frame: Vec<u8> = frame.data; // <- raw pixel y values

            let bits_written_at_start = match &mut enc {
                CodingType::ARITHMETIC(a) => a.bits_written(),
                CodingType::HUFFMAN(h) => h.bits_written(),
            };
            //let bits_written_at_start = enc.bits_written();

            match &mut enc {
                CodingType::ARITHMETIC(a) => {
                        for by in 0..blocks_per_col {
                        for bx in 0..blocks_per_row {
                            let mut diff_block = [0i16; 64];
                            
                            // Extract 8x8 block of pixel differences
                            for i in 0..8 {
                                for j in 0..8 {
                                    let y = by * 8 + i;
                                    let x = bx * 8 + j;
                                    if y < height && x < width {
                                        let idx = (y * width + x) as usize;
                                        //let diff = (current_frame[idx] as i32 - prior_frame[idx] as i32 + 256) % 256;
                                        let diff = current_frame[idx] as i16 - prior_frame[idx] as i16; // replacement for above
                                        diff_block[(i * 8 + j) as usize] = diff as i16;
                                    }
                                }
                            }
                            
                            // Apply DCT and quantization
                            let quantized = transform_and_quantize(&diff_block, &quant_table);
                            let rle = quantized_encoding(&quantized);
                            
                            // Encode RLE pairs
                            a.encode(&(rle.len() as u8), &pixel_difference_pdf, &mut bw);
                            for (run, value) in rle {
                                //clamped to stop exceeding bit range, this will probably not be perfectly lossless but idk what else would work within asignment coding context limitations
                                //let compressed_value: u8 = value.clamp(0, 255) as u8; 
                                //a.encode(&run, &pixel_difference_pdf, &mut bw);
                                let shifted_value: u8 = (value + 128).clamp(0, 255) as u8;
                                a.encode(&run, &pixel_difference_pdf, &mut bw);
                                a.encode(&shifted_value, &pixel_difference_pdf, &mut bw);
                            }
                        }
                    }
                }
                CodingType::HUFFMAN(h) => {
                    for by in 0..blocks_per_col {
                        for bx in 0..blocks_per_row {
                            let mut diff_block = [0i16; 64];
                            for i in 0..8 {
                                for j in 0..8 {
                                    let y = by * 8 + i;
                                    let x = bx * 8 + j;
                                    if y < height && x < width {
                                        let idx = (y * width + x) as usize;
                                        diff_block[(i * 8 + j) as usize] = current_frame[idx] as i16 - prior_frame[idx] as i16;
                                    }
                                }
                            }

                            let quantized: [i16; 64] = transform_and_quantize(&diff_block, &quant_table);
                            let rle = quantized_encoding(&quantized);
                            
                            let mut block_data = Vec::new();
                            block_data.push(rle.len() as u8);
                            for (run, value) in rle {
                                block_data.push(run);
                                block_data.push((value + 128).clamp(0, 255) as u8);
                            }

                            h.encode_block(&block_data, &mut bw);
                        }
                    }
                }
            }
            

            prior_frame = current_frame;
            
            let bits_written_at_end = match &mut enc {
                CodingType::ARITHMETIC(a) => a.bits_written(),
                CodingType::HUFFMAN(h) => h.bits_written(),
            };
            //let bits_written_at_end = enc.bits_written();

            if verbose {
                println!(
                    "frame: {}, compressed size (bits): {}",
                    frame.frame_num,
                    bits_written_at_end - bits_written_at_start
                );
            }
        } else {
            break;
        }
    }

    // Tie off arithmetic encoder and flush to file.
    //enc.finish(&mut bw)?;
    match &mut enc {
        CodingType::ARITHMETIC(a) => a.finish(&mut bw)?,
        CodingType::HUFFMAN(h) => h.finish(&mut bw)?,
    }
    bw.pad_to_byte()?;
    drop(bw);
    buf_writer.flush()?;
    drop(buf_writer);

    // Get original filename 
    let file_stem = input_file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");

    // Get video extension 
    let extension : &str = "mp4";

    let encoding_str = match encoding_type {
        1 => "arithmetic",
        2 => "huffman",
        _ => "unknown",
    };

    // Build filename: originalname_compression_encoding.ext
    let export_filename = format!("{}_{}_{}.{}", file_stem, quant_factor, encoding_str, extension);
    let export_path = data_folder_path.join(export_filename);

    //create ffmpeg instance for potential export
    // Ensure data folder exists (I keep getting invalid folder errors)
    fs::create_dir_all(&data_folder_path)?;

    // Ensure ffmpeg is installed
    ffmpeg_sidecar::download::auto_download().unwrap();

    // FIX: Use ffmpeg_path() instead of downloaded_ffmpeg_path()
    let ffmpeg_path = ffmpeg_sidecar::paths::ffmpeg_path();

    // Make sure output folder exists
    fs::create_dir_all(&data_folder_path)?;

    


    // Decompress and check for correctness.
    if check_decode || export_video{
        // Open the compressed output file
        let output_file = File::open(&output_file_path)?;
        let mut buf_reader = BufReader::new(output_file);
        let mut br: BitReader<_, MSB> = BitReader::new(&mut buf_reader);

        // Re-open the input video
        let iter = FfmpegCommand::new()
            .input(input_file_path.to_str().unwrap())
            .format("rawvideo")
            .pix_fmt("gray8")
            .output("-")
            .spawn()?
            .iter()?;

        // Initialize decoder
        let mut dec = match encoding_type {
            1 => DecodingType::ARITHMETIC(ArithmeticDecoder::new()),
            2 => DecodingType::HUFFMAN(HuffmanDecoder::new()),
            _ => panic!("Invalid encoding type"),
        };

        // Initial prior frame (all 128 gray)
        let mut prior_frame = vec![128u8; (width * height) as usize];
        let mut reconstructed_frames: Vec<Vec<u8>> = Vec::new();

        let block_size = 8;
        let blocks_per_row = (width + block_size - 1) / block_size;
        let blocks_per_col = (height + block_size - 1) / block_size;

        'frame_loop: for frame in iter.filter_frames() {
            if frame.frame_num < skip_count {
                continue;
            } else if frame.frame_num >= skip_count + count {
                break;
            }

            if verbose {
                println!("Decoding frame {} ...", frame.frame_num);
            }

            // Prepare current frame
            let mut current_frame = vec![0u8; (width * height) as usize];

            for by in 0..blocks_per_col {
                for bx in 0..blocks_per_row {

                    let rle_len: usize = match &mut dec {
                        DecodingType::ARITHMETIC(a) => a.decode(&pixel_difference_pdf, &mut br).to_owned() as usize,
                        DecodingType::HUFFMAN(h) => h.decode(&mut br) as usize,
                    };

                    let mut rle = Vec::with_capacity(rle_len);
                    for _ in 0..rle_len {
                        let run = match &mut dec {
                            DecodingType::ARITHMETIC(a) => a.decode(&pixel_difference_pdf, &mut br).to_owned() as usize,
                            DecodingType::HUFFMAN(h) => h.decode(&mut br) as usize,
                        };
                        let value = match &mut dec {
                            DecodingType::ARITHMETIC(a) => {
                                a.decode(&pixel_difference_pdf, &mut br).to_owned() as i16 - 128
                            },
                            DecodingType::HUFFMAN(h) => {
                                h.decode(&mut br) as i16 - 128
                            },
                        };
                        rle.push((run, value));
                    }

                    let mut coefficients = quantized_decoding(&rle);

                    for i in 0..64 {
                        let row = i / 8;
                        let col = i % 8;
                        //coefficients[i] *= DEFAULT_QUANTIZATION_TABLE[row][col] as i16;
                        //coefficients[i] = (coefficients[i] as i32 * DEFAULT_QUANTIZATION_TABLE[row][col] as i32) as i16;//might be overflowing
                        coefficients[i] = (coefficients[i] as i32 * quant_table[row][col] as i32) as i16;
                    }

                    let mut block_f32: [f32; 64] = coefficients.map(|v| v as f32);

                    transform_inverse(&mut block_f32);

                    for i in 0..64 {
                        let y = by * 8 + (i / 8);
                        let x = bx * 8 + (i % 8);
                        if y < height && x < width {
                            let idx = (y * width + x) as usize;
                            let diff = block_f32[i as usize].round() as i16;
                            current_frame[idx] = (prior_frame[idx] as i16 + diff).clamp(0, 255) as u8;
                        }
                    }
                    
                }
            }
            
            if export_video {
                //ffmpeg_stdin.write_all(&current_frame)?;//write reconstructed frame to ffmpeg for potential export / visual comparison
                reconstructed_frames.push(current_frame.clone());
            }

            // Compare with original frame
            if check_decode {
                let original_frame = &frame.data;
                for i in 0..(width * height) as usize {
                    if current_frame[i] != original_frame[i] {
                        println!(
                            "Frame {} mismatch at pixel {}: decoded {}, original {}",
                            frame.frame_num, i, current_frame[i], original_frame[i]
                        );
                        println!("Abandoning check of remaining frames");
                        break 'frame_loop;
                    }
                }
            }

            prior_frame = current_frame;

            if verbose {
                println!("Frame {} decoded correctly.", frame.frame_num);
            }
        }
        if export_video {
            // Spawn FFmpeg
            let mut ffmpeg = Command::new(ffmpeg_path)
                .args([
                    "-y",
                    "-f", "rawvideo",
                    "-pix_fmt", "gray8",
                    "-s", &format!("{}x{}", width, height),
                    "-r", "30",
                    "-i", "-",
                    "-c:v", "libx264",
                    export_path.to_str().unwrap()
                ])
                .stdin(Stdio::piped())
                .spawn()?;
                
            // Pray to god
            let ffmpeg_stdin = ffmpeg.stdin.as_mut().unwrap();
            for frame_data in &reconstructed_frames {
                ffmpeg_stdin.write_all(frame_data)?;
            }
            drop(ffmpeg_stdin);
            ffmpeg.wait()?;
            println!("Compressed video exported to {}", export_path.display());
        }
        
    }

    // Emit report
    if report {
        let total_bits = match &mut enc {
            CodingType::ARITHMETIC(a) => a.bits_written(),
            CodingType::HUFFMAN(h) => h.bits_written(),
        };
        println!(
            "{} frames encoded, average size (bits): {}, compression ratio: {:.2}",
            count,
            total_bits / count as u64,
            (width * height * 8 * count) as f64 / total_bits as f64
        )
    }


    Ok(())
}

use std::process::{Command, Stdio};

use std::fs;
use std::io::{self};


fn select_input_file(data_folder_path: &PathBuf) -> PathBuf {
    let mut entries: Vec<_> = fs::read_dir(data_folder_path)
        .expect("Failed to read data folder")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .collect();

    // Sort alphabetically for syncing purposes
    entries.sort_by_key(|e| e.file_name());

    //list all files with corresponding index
    println!("Select a file ({:?}):", data_folder_path);
    for (i, entry) in entries.iter().enumerate() {
        println!("{}. {}", i + 1, entry.file_name().to_string_lossy());
    }

    //while loop for selection
    let selection = loop {
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        match input.trim().parse::<usize>() {
            Ok(n) if n >= 1 && n <= entries.len() => break n - 1,
            _ => println!("Invalid input, enter a number between 1 and {}", entries.len()),
        }
    };

    entries[selection].path()
}

use rustdct::DctPlanner;

// Apply forward DCT to an 8x8 block of pixel differences
fn transform_and_quantize(block: &[i16; 64], quant_table: &[[u8; 8]; 8]) -> [i16; 64] {
    let mut float_block = [0.0f32; 64];
    
    // Convert to float and subtract 128 for zero-centering
    for i in 0..64 {
        float_block[i] = block[i] as f32;
    }
    
    // Apply DCT
    discrete_cosine_transform(&mut float_block);
    
    // Quantize
    let mut quantized = [0i16; 64];
    for i in 0..8 {
        for j in 0..8 {
            let idx = i * 8 + j;
            let quant = quant_table[i][j] as f32;
            quantized[idx] = (float_block[idx] / quant).round() as i16; //rounds value to nearest quant index
        }
    }
    
    quantized
}

fn discrete_cosine_transform(block: &mut [f32; 64]) {
    let mut planner = DctPlanner::new();
    let dct = planner.plan_dct2(8);

    
    
    // Apply DCT on rows
    for row in 0..8 {
        let mut row_data = [0.0f32; 8];
        for col in 0..8 {
            row_data[col] = block[row * 8 + col];
        }
        dct.process_dct2(&mut row_data);
        for col in 0..8 {
            block[row * 8 + col] = row_data[col];
        }
    }
    
    // Apply DCT on columns
    for col in 0..8 {
        let mut col_data = [0.0f32; 8];
        for row in 0..8 {
            col_data[row] = block[row * 8 + col];
        }
        dct.process_dct2(&mut col_data);
        for row in 0..8 {
            block[row * 8 + col] = col_data[row];
        }
    }

    for val in block.iter_mut() {
        *val /= 4.0; //normalizing factor
    }
}


fn quantized_encoding(coefficients: &[i16; 64]) -> Vec<(u8, i16)> {
    let mut rle = Vec::new();
    let mut run = 0u8;
    
    // Process in zigzag order
    for &coeff in coefficients.iter() {
        if coeff == 0 {
            run += 1;
        } else {
            rle.push((run, coeff));
            run = 0;
        }
    }
    
    // Handle trailing zeros
    if run > 0 {
        rle.push((run, 0));
    }
    
    rle
}

// Found Here https://stackoverflow.com/questions/29215879/how-can-i-generalize-the-quantization-matrix-in-jpeg-compression
// Based on default jpeg compression
// IDK how values are determined
const DEFAULT_QUANTIZATION_TABLE: [[u8; 8]; 8] = [
    [16, 11, 10, 16, 24, 40, 51, 61],
    [12, 12, 14, 19, 26, 58, 60, 55],
    [14, 13, 16, 24, 40, 57, 69, 56],
    [14, 17, 22, 29, 51, 87, 80, 62],
    [18, 22, 37, 56, 68, 109, 103, 77],
    [24, 35, 55, 64, 81, 104, 113, 92],
    [49, 64, 78, 87, 103, 121, 120, 101],
    [72, 92, 95, 98, 112, 100, 103, 99],
];

//for decoding dct
fn transform_inverse(block: &mut [f32; 64]) {
    let mut planner = DctPlanner::new();
    let idct = planner.plan_dct3(8);

    
    
    // Apply IDCT on columns first
    for col in 0..8 {
        let mut col_data = [0.0f32; 8];
        for row in 0..8 {
            col_data[row] = block[row * 8 + col];
        }
        idct.process_dct3(&mut col_data);
        for row in 0..8 {
            block[row * 8 + col] = col_data[row];
        }
    }
    
    // Apply IDCT on rows
    for row in 0..8 {
        let mut row_data = [0.0f32; 8];
        for col in 0..8 {
            row_data[col] = block[row * 8 + col];
        }
        idct.process_dct3(&mut row_data);
        for col in 0..8 {
            block[row * 8 + col] = row_data[col];
        }
    }

    for val in block.iter_mut() {
        *val /= 4.0; // Scale down after inverse DCT
    }
}

fn quantized_decoding(rle: &[(usize, i16)]) -> [i16; 64] {
    let mut coefficients = [0i16; 64];
    let mut pos = 0;
    
    for &(run, value) in rle {
        pos += run as usize;
        if pos < 64 {
            coefficients[pos] = value;
            pos += 1;
        }
    }
    
    coefficients
}

fn get_adjusted_quantization(factor: f32) -> [[u8; 8]; 8] {
    // if no compression then return all 1.0s
    if factor >= 100.0 {
        return [[1u8; 8]; 8];
    }

    //let factor = 100.0 - (factor - 1.0);//reverse from input for actual factoring

    //adjusts based on distance from 50 (50 is default)
    let scale = if factor < 50.0 {
        50.0 / factor
    } else {
        2.0 - factor / 50.0
    };

    let mut adjusted = [[0u8; 8]; 8];
    for i in 0..8 {
        for j in 0..8 {
            let val = (DEFAULT_QUANTIZATION_TABLE[i][j] as f32 * scale).round() as u8;
            adjusted[i][j] = val.max(1); // Ensure no zero values
        }
    }

    adjusted
}




fn parse_args(
    verbose: &mut bool,
    report: &mut bool,
    check_decode: &mut bool,
    skip_count: &mut u32,
    count: &mut u32,
    input_file_path: &mut PathBuf,
    output_file_path: &mut PathBuf,
) -> () {
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        if arg == "-verbose" {
            *verbose = true;
        } else if arg == "-no_verbose" {
            *verbose = false;
        } else if arg == "-report" {
            *report = true;
        } else if arg == "-no_report" {
            *report = false;
        } else if arg == "-check_decode" {
            *check_decode = true;
        } else if arg == "-no_check_decode" {
            *check_decode = false;
        } else if arg == "-skip_count" {
            match args.next() {
                Some(skip_count_string) => {
                    *skip_count = skip_count_string.parse::<u32>().unwrap();
                }
                None => {
                    panic!("Expected count after -skip_count option");
                }
            }
        } else if arg == "-count" {
            match args.next() {
                Some(count_string) => {
                    *count = count_string.parse::<u32>().unwrap();
                }
                None => {
                    panic!("Expected count after -count option");
                }
            }
        } else if arg == "-in" {
            match args.next() {
                Some(input_file_path_string) => {
                    *input_file_path = PathBuf::from(input_file_path_string);
                }
                None => {
                    panic!("Expected input file name after -in option");
                }
            }
        } else if arg == "-out" {
            match args.next() {
                Some(output_file_path_string) => {
                    *output_file_path = PathBuf::from(output_file_path_string);
                }
                None => {
                    panic!("Expected output file name after -out option");
                }
            }
        }
    }
}
