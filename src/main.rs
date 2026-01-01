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

use crate::frame::frame_header;

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

    blocks::process_metadata(&mut file, args.save_cover).expect("Failed to process metadata");

    // открытие битового ридера для чтения аудио фреймов из буфера файла
    let mut reader = BitReader::endian(BufReader::new(file), BigEndian);

    frame_header::FrameHeader::read_frame_header(&mut reader, stream_info);

    let _ = reader.read::<1, u8>().expect("Failed to read reserved bit");
    let subframe_type = reader.read::<6, u8>().expect("Failed to read subframe type");

    // получение типа и порядка
    let (subframe_kind, order) = match subframe_type {
        0b000000 => ("Constant", 0),
        0b000001 => ("Verbatim", 0),
        0b000010..=0b001111 => ("Fixed", subframe_type - 0x08),
        0b010000..=0b111111 => ("LPC", subframe_type - 0x20),
        _ => panic!("Invalid subframe type"),
    };

    println!("Subframe type: {}, order: {}", subframe_kind, order);
}
