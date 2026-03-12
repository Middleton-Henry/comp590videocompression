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
            _ => println!("Invalid input, please enter 1 or 2.")
        }
    };

    let mut enc = match encoding_type {
        1 => CodingType::ARITHMETIC(ArithmeticEncoder::new()),
        2 => CodingType::HUFFMAN(HuffmanEncoder::new()),
        _ => panic!("Invalid encoding type")
    };

    

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
    let mut pixel_difference_pdf = VectorCountSymbolModel::new((0..=255).collect());

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
                    // Process pixels in row major order.
                    for r in 0..height {
                        for c in 0..width {
                            let pixel_index = (r * width + c) as usize;

                            // Encode difference with same pixel in prior frame.
                            // Normalize and modulate difference to 8-bit range.
                            let pixel_difference = (((current_frame[pixel_index] as i32)
                                - (prior_frame[pixel_index] as i32))
                                + 256)
                                % 256;
                            

                            a.encode(&pixel_difference, &pixel_difference_pdf, &mut bw);

                            // Update context
                            pixel_difference_pdf.incr_count(&pixel_difference);
                        }
                    }
                }
                CodingType::HUFFMAN(h) => {
                    let mut diff_frame = vec![0u8; (width * height) as usize];
                    for i in 0..diff_frame.len() {
                        diff_frame[i] = (((current_frame[i] as i32 - prior_frame[i] as i32) + 256) % 256) as u8;
                    }
                    h.encode(&diff_frame, &mut bw);
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
    buf_writer.flush()?;

    // Decompress and check for correctness.
    if check_decode {
        match encoding_type {
            1 => {
                let output_file = match File::open(&output_file_path) {
                    Err(_) => panic!("Error opening output file"),
                    Ok(f) => f,
                };
                let mut buf_reader = BufReader::new(output_file);
                let mut br: BitReader<_, MSB> = BitReader::new(&mut buf_reader);

                let iter = FfmpegCommand::new() // <- Builder API like `std::process::Command`
                    .input(input_file_path.to_str().unwrap())
                    .format("rawvideo")
                    .pix_fmt("gray8")
                    .output("-")
                    .spawn()? // <- Ordinary `std::process::Child`
                    .iter()?; // <- Blocking iterator over logs and output

                let mut dec = DecodingType::ARITHMETIC(ArithmeticDecoder::new());

                // kinda redundant due to presence in in iff statement, but easier to paste
                let decoded_pixel_difference = match &mut dec {
                    DecodingType::ARITHMETIC(a) => a.decode(&pixel_difference_pdf, &mut br).to_owned(),//needs to be cloned for whatever reason
                    DecodingType::HUFFMAN(h) => h.decode(&mut br) as i32 //might be using unnecessary bits by casting to i32
                };

                // Set up initial prior frame as uniform medium gray
                let mut prior_frame = vec![128 as u8; (width * height) as usize];

                'outer_loop: 
                for frame in iter.filter_frames() {
                    if frame.frame_num < skip_count + count {
                        if verbose {
                            print!("Checking frame: {} ... ", frame.frame_num);
                        }

                        let current_frame: Vec<u8> = frame.data; // <- raw pixel y values

                        // Process pixels in row major order.
                        for r in 0..height {
                            for c in 0..width {
                                let pixel_index = (r * width + c) as usize;
                                let decoded_pixel_difference = match &mut dec {
                                    DecodingType::ARITHMETIC(a) => a.decode(&pixel_difference_pdf, &mut br).to_owned(),//needs to be cloned for whatever reason
                                    DecodingType::HUFFMAN(h) => h.decode(&mut br) as i32
                                };
                                pixel_difference_pdf.incr_count(&decoded_pixel_difference);

                                let pixel_value = (prior_frame[pixel_index] as i32 + decoded_pixel_difference) % 256;

                                if pixel_value != current_frame[pixel_index] as i32 {
                                    println!(
                                        " error at ({}, {}), should decode {}, got {}",
                                        c, r, current_frame[pixel_index], pixel_value
                                    );
                                    println!("Abandoning check of remaining frames");
                                    break 'outer_loop;
                                }
                            }
                        }
                        println!("correct.");
                        prior_frame = current_frame;
                    } else {
                        break 'outer_loop;
                    }
                }
            }
            2 => {
                let output_file = match File::open(&output_file_path) {
                    Err(_) => panic!("Error opening output file"),
                    Ok(f) => f,
                };
                let mut buf_reader = BufReader::new(output_file);
                let mut br: BitReader<_, MSB> = BitReader::new(&mut buf_reader);

                let iter = FfmpegCommand::new() // <- Builder API like `std::process::Command`
                    .input(input_file_path.to_str().unwrap())
                    .format("rawvideo")
                    .pix_fmt("gray8")
                    .output("-")
                    .spawn()? // <- Ordinary `std::process::Child`
                    .iter()?; // <- Blocking iterator over logs and output

                // Initialize adaptive Huffman decoder
                let mut dec = DecodingType::HUFFMAN(HuffmanDecoder::new());

                // Initial uniform gray frame
                let mut prior_frame = vec![128u8; (width * height) as usize];

                'outer_loop: 
                for frame in iter.filter_frames() {
                    if frame.frame_num >= skip_count + count {
                        break 'outer_loop;
                    }

                    if verbose {
                        print!("Checking frame: {} ... ", frame.frame_num);
                    }

                    let current_frame: Vec<u8> = frame.data;

                    for r in 0..height {
                        for c in 0..width {
                            let pixel_index = (r * width + c) as usize;

                            let decoded_pixel_difference = match &mut dec {
                                DecodingType::ARITHMETIC(a) => a.decode(&pixel_difference_pdf, &mut br).to_owned(),//needs to be cloned for whatever reason
                                DecodingType::HUFFMAN(h) => h.decode(&mut br) as i32 //might be using unnecessary bits by casting to i32
                            };

                            // Reconstruct actual pixel
                            let pixel_value = (prior_frame[pixel_index] as i32 + decoded_pixel_difference as i32) % 256;

                            if pixel_value != current_frame[pixel_index] as i32 {
                                println!(
                                    " error at ({}, {}), should decode {}, got {}",
                                    c, r, current_frame[pixel_index], pixel_value
                                );
                                println!("Abandoning check of remaining frames");
                                break 'outer_loop;
                            }
                        }
                    }

                    if verbose {
                        println!("correct.");
                    }

                    prior_frame = current_frame;
                }
            }
            _ => {
                panic!("Invalid encoding type");
            }
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
use std::f32::consts::SQRT_2;

//Splits frames into 8x8 chunks
fn apply_dct_block(block: &mut [f32]) {
    let mut planner = DctPlanner::new();
    let dct = planner.plan_dct2(8); // 1D DCT
    // Apply DCT on rows
    for row in 0..8 {
        dct.process_dct2(&mut block[row*8..(row+1)*8]);
    }
    // Apply DCT on columns
    let mut col = [0f32; 8];
    for c in 0..8 {
        for r in 0..8 { col[r] = block[r*8 + c]; }
        dct.process_dct2(&mut col);
        for r in 0..8 { block[r*8 + c] = col[r]; }
    }
}

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
