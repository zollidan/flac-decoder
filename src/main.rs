#![warn(clippy::all, clippy::pedantic)]

// docs : https://www.rfc-editor.org/rfc/rfc9639.html#name-examples

use std::env;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};

use bitstream_io::{BigEndian, BitRead, BitReader};

mod stream_info;
mod picture;
mod metedata_blocks;

#[derive(Debug)]
pub struct FrameHeader {
    pub sync_code: u16,
    pub blocking_strategy: u8,
    pub block_size_code: u8,
    pub sample_rate: f32,
    pub channel_assignment: String,
    pub bit_depth: u32,
    pub mandatory: u8,
    pub frame_or_sample_number: u64,
    pub block_size: u16,
    pub crc8: u8,
}

pub struct Frame {
    pub header: FrameHeader,
    pub subframes: Vec<Subframe>,
}

struct SubframeHeader {}

pub struct Subframe {
    subframe_header: SubframeHeader,
}

// функция для чтения переменной длины UTF-8 закодированного u64
fn read_utf8_u64<R: Read>(reader: &mut BitReader<R, BigEndian>) -> std::io::Result<u64> {
    let mut val = reader.read::<8, u8>()? as u64;
    let mut mask = 0x80;
    let mut len = 0;

    // определяем количество дополнительных байт по количеству ведущих единиц
    while (val & mask) != 0 {
        len += 1;
        mask >>= 1;
    }

    if len == 1 || len > 7 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid UTF-8 sequence",
        ));
    }

    if len == 0 {
        return Ok(val); // число < 128
    }

    // оставляем только полезные биты из первого байта
    val &= mask - 1;

    for _ in 0..(len - 1) {
        let byte = reader.read::<8, u8>()? as u64;
        if (byte & 0xC0) != 0x80 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid UTF-8 continuation",
            ));
        }
        val = (val << 6) | (byte & 0x3F);
    }

    Ok(val)
}

fn check_flac_header(file: &mut File) -> io::Result<()> {
    let mut format_part = [0u8; 4];
    file.read_exact(&mut format_part)?;
    if &format_part != b"fLaC" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Not a FLAC file",
        ));
    }
    Ok(())
}

// функция для поиска количества битов, отведенных под убитые биты
// хорошо бы потом сделать -> Result<u32, std::io::Error>
fn find_wasted_bits(reader: &mut BitReader<BufReader<File>, BigEndian>) -> u32 {
    let wasted_bits_flag = reader.read::<1, u8>().unwrap();
    let mut k = 0;
    if wasted_bits_flag == 1 {
        while reader.read::<1, u8>().unwrap() == 0 {
            k += 1;
        }
        k += 1;
    };

    k
}

fn constant_value() {}

fn verbatim() {}

fn fixed_prediction(
    reader: &mut BitReader<BufReader<File>, BigEndian>,
    order: u8,
    bps: u8,
    block_size: u32,
) -> Vec<u64> {
    // создаю вектор для хранения сэмплов в подфрейме
    let mut samples = vec![0u64; block_size as usize];

    // в длину порядка читаю прогревочные семплы
    // заменить 16 на bps -> bits per sample
    // !!!!
    for i in 0..order as usize {
        // плюс перевести на signed 
        samples[i] = reader.read::<16, u64>().unwrap();
    }

    // декодирую residual он же остаток
    let residual = decode_rice_residual(reader, order, block_size);

    // применяю предсказание для каждого сэмпла начиная с order до конца блока
    // тест для работы с индексами вектора так как при n = 0 будет ошибка
    for n in order as usize..block_size as usize {
        let prediction = match order {
            // 0
            0 => 0,
            // a(n-1)
            1 => samples[n - 1],
            // 2 * a(n-1) - a(n-2)
            2 => 2 * samples[n - 1] - samples[n - 2],
            // 3 * a(n-1) - 3 * a(n-2) + a(n-3)
            3 => 3 * samples[n - 1] - 3 * samples[n - 2] + samples[n - 3],
            // 4 * a(n-1) - 6 * a(n-2) + 4 * a(n-3) - a(n -4)
            4 => 4 * samples[n - 1] - 6 * samples[n - 2] + 4 * samples[n - 3] - samples[n - 4],
            _ => unreachable!(),
        };
    }

    samples
}

fn decode_rice_residual() {}

fn lpc() {}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: cargo run <flac_file>");
        return;
    }

    let path = &args[1];

    let mut file = File::open(path).unwrap();

    check_flac_header(&mut file).expect("Error validating flac header");

    stream_info::StreamInfo::process_stream_info_block(&mut file);

    metedata_blocks::process_metadata(&mut file).unwrap();

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
