#![warn(clippy::all, clippy::pedantic)]

// docs : https://www.rfc-editor.org/rfc/rfc9639.html#name-examples

use std::env;
use std::fs::File;
use std::io::BufReader;

use bitstream_io::{BigEndian, BitRead, BitReader};

mod metadata;
mod frame;
mod picture;

use metadata::stream_info::{self, StreamInfo};
use metadata::blocks;
use frame::header::{FrameHeader, read_utf8_u64};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: cargo run <flac_file>");
        return;
    }

    let path = &args[1];

    let mut file = File::open(path).unwrap();

    stream_info::check_flac_header(&mut file).expect("Error validating flac header");

    let steam_info = StreamInfo::process_stream_info_block(&mut file);

    blocks::process_metadata(&mut file).unwrap();

    // открытие битового ридера для чтения аудио фреймов из буфера файла
    let mut reader = BitReader::endian(BufReader::new(file), BigEndian);

    // чтение синхронизирующего кода из аудио фрейма
    // 14 бит (не 15!)
    // всегда должно быть 0b11111111111110
    let sync_code = reader.read::<14, u16>().expect("Sync error");
    if sync_code != 0x3FFE {
        panic!("Lost sync");
    }

    // 1 бит - reserved
    // должен быть 0
    let _reserved = reader.read::<1, u8>().unwrap();

    // 1 бит
    let blocking_strategy = reader.read::<1, u8>().unwrap();

    // 4 бита
    let block_size_bits = reader.read::<4, u8>().unwrap();

    // обработка block_size
    let mut block_size: u16 = match block_size_bits {
        0b0000 => panic!("Reserved"),
        0b0001 => 192,
        0b0010..=0b0101 => 576 << (block_size_bits - 0b0010),
        0b0110 => 0, // будет прочитано позже
        0b0111 => 0, // будет прочитано позже
        0b1000..=0b1111 => 1 << block_size_bits,
        _ => unreachable!(),
    };

    // 4 бита - sample rate
    let sample_rate_bits = reader.read::<4, u8>().unwrap();

    // обработка sample_rate
    let mut sample_rate = match sample_rate_bits {
        0b0000 => steam_info.sample_rate as f32 / 1000.0, // взять из streaminfo
        0b0001 => 88.2,
        0b0010 => 176.4,
        0b0011 => 192.0,
        0b0100 => 8.0,
        0b0101 => 16.0,
        0b0110 => 22.05,
        0b0111 => 24.0,
        0b1000 => 32.0,
        0b1001 => 44.1,
        0b1010 => 48.0,
        0b1011 => 96.0,
        0b1100 => 0.0, // будет прочитано позже
        0b1101 => 0.0, // будет прочитано позже
        0b1110 => 0.0, // будет прочитано позже
        0b1111 => panic!("Forbidden"),
        _ => unreachable!(),
    };

    // 4 бита - channel assignment
    let channel_assignment_bits = reader.read::<4, u8>().unwrap();

    // обработка channel_assignment
    let channel_assignment = match channel_assignment_bits {
        0b0000 => "1 channel: mono",
        0b0001 => "2 channels: left, right",
        0b0010 => "3 channels: left, right, center",
        0b0011 => "4 channels: front left, front right, back left, back right",
        0b0100 => {
            "5 channels: front left, front right, front center, back/surround left, back/surround right"
        }
        0b0101 => {
            "6 channels: front left, front right, front center, LFE, back/surround left, back/surround right"
        }
        0b0110 => {
            "7 channels: front left, front right, front center, LFE, back center, side left, side right"
        }
        0b0111 => {
            "8 channels: front left, front right, front center, LFE, back left, back right, side left, side right"
        }
        0b1000 => "2 channels: left, right; stored as left-side stereo",
        0b1001 => "2 channels: left, right; stored as side-right stereo",
        0b1010 => "2 channels: left, right; stored as mid-side stereo",
        0b1011..=0b1111 => "reserved",
        _ => unreachable!("Value from 4 bits cannot exceed 15"),
    };

    // 3 бита - bit depth
    let bit_depth_bits = reader.read::<3, u8>().unwrap();

    // обработка bit_depth
    let bit_depth = match bit_depth_bits {
        0b000 => steam_info.bps as u32, // взять из streaminfo
        0b001 => 8,
        0b010 => 12,
        0b011 => panic!("Reserved"),
        0b100 => 16,
        0b101 => 20,
        0b110 => 24,
        0b111 => 32,
        _ => unreachable!(),
    };

    // 1 бит - mandatory (должен быть 0)
    let mandatory = reader.read::<1, u8>().unwrap();

    // чтение frame/sample number
    // читаю из UTF-8 переменной длины
    let frame_or_sample_number = read_utf8_u64(&mut reader).unwrap();

    // дочитываем block_size если нужно
    if block_size_bits == 0b0110 {
        block_size = reader.read::<8, u16>().unwrap() + 1;
    } else if block_size_bits == 0b0111 {
        block_size = reader.read::<16, u16>().unwrap() + 1;
    }

    // дочитываю sample_rate если нужно
    // переместить в отдельную функцию потом
    // лучше бы вообще в impl
    if sample_rate_bits == 0b1100 {
        sample_rate = reader.read::<8, u8>().unwrap() as f32; // в kHz
    } else if sample_rate_bits == 0b1101 {
        sample_rate = reader.read::<16, u16>().unwrap() as f32 / 1000.0; // хранится в файле как Hz, конвертируем в kHz
    } else if sample_rate_bits == 0b1110 {
        sample_rate = reader.read::<16, u16>().unwrap() as f32 / 10.0 / 1000.0; // хранится в файле как Hz/10, конвертируем в kHz
    }

    // CRC-8
    let crc8 = reader.read::<8, u8>().unwrap();

    let frame_header = FrameHeader {
        sync_code,
        blocking_strategy,
        block_size_code: block_size_bits,
        sample_rate,
        channel_assignment: channel_assignment.to_string(),
        bit_depth,
        mandatory,
        frame_or_sample_number,
        block_size,
        crc8,
    };
    println!("{:#?}", frame_header);

    println!("Subframe count: {}", steam_info.channels);

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

    // вызов конкретных функций декодирования в зависимости от типа сабфрейма
    // может быть добавить в последний match
}
