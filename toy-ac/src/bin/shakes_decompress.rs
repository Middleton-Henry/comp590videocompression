use std::env;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

use bitbit::{BitReader, MSB};
use toy_ac::arithmetic_decoder::Decoder;

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
    let mut dec = Decoder::new();

    let mut data_folder_path = get_workspace_root();
    data_folder_path.push("data");

    let input_file = match File::open(data_folder_path.join("out.dat")) {
        Err(_) => panic!("Error opening file"),
        Ok(f) => f,
    };

    let output_file = match File::create(data_folder_path.join("reconstructed.txt")) {
        Err(_) => panic!("Error opening output file"),
        Ok(f) => f,
    };

    let mut log_writer: Option<BufWriter<File>>;
    if log_flag {
        let log_file = match File::create(data_folder_path.join("decode-log.txt")) {
            Err(_) => panic!("Error opening log file"),
            Ok(f) => f,
        };
        log_writer = Some(BufWriter::new(log_file));
    } else {
        log_writer = None;
    }

    let mut buf_reader = BufReader::new(input_file);

    let mut size_bytes: [u8; 8] = [0; 8];
    buf_reader.read(&mut size_bytes)?;
    let output_size = u64::from_be_bytes(size_bytes);

    let mut br: BitReader<_, MSB> = BitReader::new(&mut buf_reader);

    let mut writer = BufWriter::new(output_file);

    for count in 0..output_size {
        if log_flag {
            let mut lw = log_writer.unwrap();
            write!(
                &mut lw,
                "Count: {}, High: {:#x}, Low: {:#010x}, Buffer: {:#010x}, Total: {:10}, ",
                count,
                dec.high(),
                dec.low(),
                dec.buffer(),
                sm.total()
            )?;
            log_writer = Some(lw);
        }

        let next_byte = dec.decode(&sm, &mut br);
        let next_byte = next_byte.to_owned();
        sm.incr_count(&next_byte);

        if log_flag {
            let mut lw = log_writer.unwrap();
            write!(
                &mut lw,
                "Symbol: {}, High: {:#x}, Low: {:#010x}\n",
                if next_byte == 10 {
                    format!("\\n ")
                } else {
                    format!("'{}'", (next_byte as char))
                },
                dec.high(),
                dec.low()
            )?;
            log_writer = Some(lw);
        }

        writer.write(&[next_byte])?;
    }

    writer.flush()?;

    if log_flag {
        let mut lw = log_writer.unwrap();
        lw.flush()?;
    }

    Ok(())
}
