#![warn(clippy::all, clippy::pedantic)]

// docs : https://www.rfc-editor.org/rfc/rfc9639.html#name-examples

use std::fs::File;
use std::io::BufReader;

use bitstream_io::{BigEndian, BitRead, BitReader};
use clap::Parser;

mod frame;
mod metadata;

use metadata::blocks;
use metadata::stream_info::{self, StreamInfo};

use crate::frame::{frame_header, subframe};

/// FLAC decoder with metadata extraction
#[derive(Parser, Debug)]
#[command(name = "flac-decoder")]
#[command(about = "Decode FLAC files and extract metadata", long_about = None)]
struct Args {
    /// Path to the FLAC file
    #[arg(value_name = "FILE")]
    file: String,

    /// Save cover art from metadata to file
    #[arg(short, long)]
    save_cover: bool,
}

fn main() {
    let args = Args::parse();
    let path = &args.file;

    let mut file = File::open(path).expect("Failed to open file");

    stream_info::check_flac_header(&mut file).expect("Error validating flac header");

    let stream_info = StreamInfo::process_stream_info_block(&mut file);

    println!("Stream Info: {:#?}", stream_info);

    blocks::process_metadata(&mut file, args.save_cover).expect("Failed to process metadata");

    // открытие битового ридера для чтения аудио фреймов из буфера файла
    let mut reader = BitReader::endian(BufReader::new(file), BigEndian);
    
    let mut const_encode = 0;
    let mut verbatim_encode = 0;
    let mut fixed_encode = 0;
    let mut lpc_encode = 0;

    loop {
        let frame_header = match frame_header::FrameHeader::read_frame_header(&mut reader, &stream_info) {
            Ok(header) => header,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                println!("Достигнут конец файла.");
                break;
            }
            Err(e) => {
                eprintln!("Ошибка при чтении фрейма: {}", e);
                break;
            }
        };
        
        for _ in 0..stream_info.channels {
            let _ = reader.read::<1, u8>().expect("Failed to read reserved bit");
            let subframe_type = reader
                .read::<6, u8>()
                .expect("Failed to read subframe type");

            let wasted_bits = subframe::find_wasted_bits(&mut reader).expect("Error find wasted bits");

            match subframe_type {
                0b000000 => {
                    const_encode += 1;
                    let _ = reader.read::<24, u32>();
                 },
                0b000001 => {
                    verbatim_encode += 1;
                 },
                0b000010..=0b001111 => {
                    fixed_encode += 1;
                 },
                0b010000..=0b111111 => {
                    lpc_encode += 1;
                 },
                _ => panic!("Invalid subframe type"),
            };
        }

        reader.byte_align();
        let _ = reader.read::<16, u16>().expect("Failed to read CRC");
    };

    print!("Subframe Encoding Methods Usage:\n") ;
    print!("Constant Encoding: {} times\n", const_encode);
    print!("Verbatim Encoding: {} times\n", verbatim_encode);
    print!("Fixed Prediction Encoding: {} times\n", fixed_encode);
    print!("LPC Encoding: {} times\n", lpc_encode);

}
