#![warn(clippy::all, clippy::pedantic)]

// docs : https://www.rfc-editor.org/rfc/rfc9639.html#name-examples

use std::env;
use std::fs::File;
use std::io::BufReader;

use bitstream_io::{BigEndian, BitRead, BitReader};

mod frame;
mod metadata;

use metadata::blocks;
use metadata::stream_info::{self, StreamInfo};

use crate::frame::frame_header;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: cargo run <flac_file>");
        return;
    }

    let path = &args[1];

    let mut file = File::open(path).unwrap();

    stream_info::check_flac_header(&mut file).expect("Error validating flac header");

    let stream_info = StreamInfo::process_stream_info_block(&mut file);

    blocks::process_metadata(&mut file).unwrap();

    // открытие битового ридера для чтения аудио фреймов из буфера файла
    let mut reader = BitReader::endian(BufReader::new(file), BigEndian);

    frame_header::FrameHeader::read_frame_header(&mut reader, stream_info);

    let _ = reader.read::<1, u8>().unwrap();
    let subframe_type = reader.read::<6, u8>().unwrap();

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
