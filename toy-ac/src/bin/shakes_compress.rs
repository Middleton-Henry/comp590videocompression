use std::env;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

use bitbit::BitWriter;
use toy_ac::arithmetic_encoder::Encoder;

use toy_ac::symbol_model::SymbolModel;
use toy_ac::symbol_model::VectorCountSymbolModel;
use workspace_root::get_workspace_root;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut log_flag = false;
    for arg in env::args().skip(1) {
        if arg == "-log" {
            log_flag = true;
        }
    }

    let mut sm = VectorCountSymbolModel::new((0..=255).collect());
    let mut enc = Encoder::new();

    let mut data_folder_path = get_workspace_root();
    data_folder_path.push("data");

    let input_file = match File::open(data_folder_path.join("shakespeare.txt")) {
        Err(_) => panic!("Error opening file"),
        Ok(f) => f,
    };
    let metadata = input_file.metadata()?;
    let input_length = metadata.len();

    let output_file = match File::create(data_folder_path.join("out.dat")) {
        Err(_) => panic!("Error opening output file"),
        Ok(f) => f,
    };

    let mut log_writer: Option<BufWriter<File>>;
    if log_flag {
        let log_file = match File::create(data_folder_path.join("encode-log.txt")) {
            Err(_) => panic!("Error opening log file"),
            Ok(f) => f,
        };
        log_writer = Some(BufWriter::new(log_file));
    } else {
        log_writer = None;
    }

    let mut buf_writer = BufWriter::new(output_file);
    // First write out the input length as a u64
    buf_writer.write(&input_length.to_be_bytes())?;

    let mut bw = BitWriter::new(&mut buf_writer);

    let reader = BufReader::new(input_file);

    let mut count = 0;

    for next_byte in reader.bytes() {
        if log_flag {
            let mut lw = log_writer.unwrap();
            write!(
                &mut lw,
                "Count: {}, High: {:#x}, Low: {:#010x}, ",
                count,
                enc.high(),
                enc.low()
            )?;
            log_writer = Some(lw);
        }

        match next_byte {
            Ok(b) => {
                if log_flag {
                    let (int_start, int_end) = sm.interval(&b);        

                    let mut lw = log_writer.unwrap();
                    write!(
                        &mut lw,
                        "Symbol: {}, IntStart: {:10}, IntEnd: {:10}, Total: {:10}, ",
                        if b == 10 {format!("\\n ")} else {format!("'{}'", (b as u8 as char))},  
                        int_start, int_end, sm.total()
                    )?;
                    log_writer = Some(lw);
                }

                enc.encode(&b, &sm, &mut bw);
                sm.incr_count(&b);

                if log_flag {
                    let mut lw = log_writer.unwrap();
                    write!(
                        &mut lw,
                        "High: {:#x}, Low: {:#010x}\n",
                        enc.high(), enc.low()
                    )?;
                    log_writer = Some(lw);
                }
            }
            Err(_) => panic!("Error reading byte from file"),
        }
        count += 1;
    }

    enc.finish(&mut bw)?;

    bw.pad_to_byte()?;
    buf_writer.flush()?;

    if log_flag {
        let mut lw = log_writer.unwrap();
        lw.flush()?;
    }

    Ok(())
}
