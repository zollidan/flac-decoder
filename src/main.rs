#![warn(clippy::all, clippy::pedantic)]

use std::fs::File;
use std::io::{self, BufReader, Cursor, Read, Seek, SeekFrom};

use bitstream_io::{BigEndian, BitRead, BitReader};
use image::ImageReader;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum BitDepth {
    FromStreamInfo, // 0b000
    Bits8,          // 0b001
    Bits12,         // 0b010
    Reserved,       // 0b011
    Bits16,         // 0b100
    Bits20,         // 0b101
    Bits24,         // 0b110
    Bits32,         // 0b111
}

impl BitDepth {
    pub fn from_u8(value: u8) -> Self {
        match value & 0x07 {
            0b000 => Self::FromStreamInfo,
            0b001 => Self::Bits8,
            0b010 => Self::Bits12,
            0b100 => Self::Bits16,
            0b101 => Self::Bits20,
            0b110 => Self::Bits24,
            0b111 => Self::Bits32,
            _ => Self::Reserved,
        }
    }

    pub fn bits(&self) -> Option<u8> {
        match self {
            Self::Bits8 => Some(8),
            Self::Bits12 => Some(12),
            Self::Bits16 => Some(16),
            Self::Bits20 => Some(20),
            Self::Bits24 => Some(24),
            Self::Bits32 => Some(32),
            _ => None,
        }
    }
}

// поменять потом на расширеную версию с количеством каналов
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ChannelAssignment {
    Independent(u8),    // 0b0000-0b0111 от 1 до 8 независимых каналов
    LeftSideStereo,     // 0b1000: Left + Side
    SideRightStereo,    // 0b1001: Side + Right
    MidSideStereo,      // 0b1010: Mid + Side
    Reserved,           // 0b1011-0b1111
}

impl ChannelAssignment {
    fn from_u8(value: u8) -> Self {
        match value {
            v @ 0..=7 => Self::Independent(v + 1),
            0b1000 => Self::LeftSideStereo,
            0b1001 => Self::SideRightStereo,
            0b1010 => Self::MidSideStereo,
            _ => Self::Reserved,
        }
    }
}

#[derive(Debug, PartialEq)]
enum SampleRate {
    FromStreamInfo, // 0b0000
    KHz88_2,        // 0b0001
    KHz176_4,       // 0b0010
    KHz192,         // 0b0011
    KHz8,           // 0b0100
    KHz16,          // 0b0101
    KHz22_05,       // 0b0110
    KHz24,          // 0b0111
    KHz32,          // 0b1000
    KHz44_1,        // 0b1001
    KHz48,          // 0b1010
    KHz96,          // 0b1011
    Uncommon8bit,   // 0b1100
    Uncommon16bit,  // 0b1101
    Uncommon16bitDiv10, // 0b1110
    Forbidden,      // 0b1111
}

impl SampleRate {
    fn from_u8(value: u8) -> Self {
        match value {
            0b0000 => Self::FromStreamInfo,
            0b0001 => Self::KHz88_2,
            0b0010 => Self::KHz176_4,
            0b0011 => Self::KHz192,
            0b0100 => Self::KHz8,
            0b0101 => Self::KHz16,
            0b0110 => Self::KHz22_05,
            0b0111 => Self::KHz24,
            0b1000 => Self::KHz32,
            0b1001 => Self::KHz44_1,
            0b1010 => Self::KHz48,
            0b1011 => Self::KHz96,
            0b1100 => Self::Uncommon8bit,
            0b1101 => Self::Uncommon16bit,
            0b1110 => Self::Uncommon16bitDiv10,
            _ => Self::Forbidden,
        }
    }
}

#[derive(Debug)]
struct FrameHeader {
    sync_code: u16,
    blocking_strategy: u8,
    block_size_code: u8,
    sample_rate_code: SampleRate,
    channel_assignment: ChannelAssignment,
    bit_depth: BitDepth,
    mandatory: u8,
    frame_or_sample_number: u64, 
    block_size: u8,
    crc8: u8,
}

struct Frame {
    header: FrameHeader,
    subframes: Vec<Subframe>,
}

struct Subframe {
    // данные субфрейма
}

// docs : https://www.rfc-editor.org/rfc/rfc9639.html#name-examples

#[derive(Debug)]
struct StreamInfo {
    min_block_size: u16,
    max_block_size: u16,
    min_frame_size: u32,
    max_frame_size: u32,
    sample_rate: u64,
    channels: u8,
    bps: u8,
    total_samples: u64,
    checksum_combined: [u8; 16],
}

#[derive(Debug)]
struct PictureBlock{
    picture_type: u32,
    media_type: String,
    description_length: u32,
    width: u32,
    height: u32,
    color_depth: u32,
    colors_used: u32,
    picture_data_length: u32,
}

fn check_flac_header(file: &mut File) -> io::Result<()> {
    let mut format_part = [0u8; 4];
    file.read_exact(&mut format_part)?;
    if &format_part != b"fLaC" {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Not a FLAC file"));
    }
    Ok(())
}

fn get_header(file: &mut File) -> (bool, u8, u32) {
    let mut header = [0u8; 4];
    file.read_exact(&mut header);

    // побитовая операция
    // первый бит 0 или 1 если 0 то это не последний блок метаданных
    // следующие 7 бит - тип блока 0 - STREAMINFO 1 - PADDING и тд
    let is_last = (header[0] & 0x80) != 0;
    let block_type = header[0] & 0x7F;

    // следующие 3 байта - длина блока метаданных
    // собираю 24 бита из 3 байт
    // сдвигаю первый байт на 16 бит влево, второй на 8 бит и добавляю третий
    let length = ((header[1] as u32) << 16) |
                ((header[2] as u32) << 8)  |
                (header[3] as u32);


    (is_last, block_type, length)
}

// получение и сохранение картинки из метаданных
fn process_picture_block(picture_block: Vec<u8>) {

    let mut step = 0;

    let picture_type = u32::from_be_bytes(picture_block[step..step+4].try_into().unwrap());
    step += 4;
    
    let media_type_length = u32::from_be_bytes(picture_block[step..step+4].try_into().unwrap());
    step += 4;

    let media_type = std::str::from_utf8(&picture_block[step..step + media_type_length as usize]).unwrap();
    step += media_type_length as usize;

    let description_length = u32::from_be_bytes(picture_block[step..step+4].try_into().unwrap());
    step += 4;
    step += description_length as usize;
    
    let mut width = u32::from_be_bytes(picture_block[step..step+4].try_into().unwrap());
    step += 4;
    let mut height = u32::from_be_bytes(picture_block[step..step+4].try_into().unwrap());
    step += 4;
    let color_depth = u32::from_be_bytes(picture_block[step..step+4].try_into().unwrap());
    step += 4;
    let colors_used = u32::from_be_bytes(picture_block[step..step+4].try_into().unwrap());
    step += 4;
    let picture_data_length = u32::from_be_bytes(picture_block[step..step+4].try_into().unwrap());
    step += 4;
    let picture_data = &picture_block[step..step + picture_data_length as usize];

    // сохранение картинки в файл
    let file_name = format!("picture_{}.{}", picture_type, match media_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        _ => "bin",
    });

    let cursor = Cursor::new(picture_data);
    
    match ImageReader::new(cursor).with_guessed_format() {
        Ok(reader) => {
            match reader.decode() { 
                Ok(image) => {
                    if width == 0 || height == 0 {
                        width = image.width();
                        height = image.height();
                    }
                    match image.save(&file_name) {
                        Ok(_) => println!("Saved picture to {}", file_name),
                        Err(e) => println!("Failed to save picture: {}", e),
                    }
                }
                Err(e) => println!("Failed to decode image: {}", e),
            }
        }
        Err(e) => {
                println!("Failed to read image dimensions: {}", e);
        }
    }

    let picture = PictureBlock{
        picture_type,
        media_type: media_type.to_string(),
        description_length,
        width,
        height,
        color_depth,
        colors_used,
        picture_data_length,
    };

    println!("{:#?}", picture);


}

fn process_metadata(file: &mut File) -> io::Result<()> {
    // скип остальных блоков метаданных
    /*
    0	Streaminfo
    1	Padding
    2	Application
    3	Seek table
    4	Vorbis comment
    5	Cuesheet
    6	Picture
    */
    loop {
        let (is_last, block_type, length) = get_header(file);

        // пока работает только обработка блока картинки
        match block_type {
            // блок картинки
            6 => {
                let mut buffer = vec![0u8; length as usize];
                file.read_exact(&mut buffer)?;
                process_picture_block(buffer);
            }
            _ => {
                // пропускаем остальные блоки
                file.seek(SeekFrom::Current(length as i64))?;
            }
        }

        if is_last {
            break;
        }
    }
    Ok(())
}

fn main() {
    let path = "song.flac";

    let mut file = File::open(path).unwrap();

    check_flac_header(&mut file);

    let streaminfo_header = get_header(&mut file);

    // первый всегда идет STREAMINFO
    // поменять потом с индексов на именованные поля
    if streaminfo_header.1 != 0 {
        panic!("Expect STREAMINFO (type 0)");
    }

    // создаю вектор в длину блока и читаю его содержимое
    let mut streaminfo = vec![0u8; streaminfo_header.2 as usize];
    file.read_exact(&mut streaminfo).unwrap();

    // чтение информация из STREAMINFO
    // собираю значения из байт массива согласно докам
    // TODO: переписать на from_be_bytes где возможно
    let min_block_size = u16::from_be_bytes(streaminfo[0..2].try_into().unwrap());
    let max_block_size = u16::from_be_bytes(streaminfo[2..4].try_into().unwrap());
    let min_frame_size = u32::from_be_bytes([0, streaminfo[4], streaminfo[5], streaminfo[6]]);
    let max_frame_size = u32::from_be_bytes([0, streaminfo[7], streaminfo[8], streaminfo[9]]);
    // беру сразу 8 байт с 10 по 17 и комбинирую в одно 64 битное число
    // так как дальше идут значения которые занимают биты в этих байтах
    // так удобнее всего двигаться внутри байтов 
    let combinated = u64::from_be_bytes(streaminfo[10..18].try_into().unwrap());
    // получение 16 байт контрольной суммы MD5
    let checksum_combined: [u8; 16] = streaminfo[18..34].try_into().unwrap();
    // так как значение занимает 20 то сдвигаю на 12 бита вправо от 32 и маской беру 20 бит
    let sample_rate = (combinated >> 44) & 0xFFFFF; // 20 bit
    // сдвигаю от 32 на 9 бит и маской беру 3 бита
    let channels = (combinated >> 41)  & 0x7;    // 3 bit
    // сдвигаю от 32 на 4 бит и маской беру 5 бит
    let bps = (combinated >> 36)  & 0x1F;   // 5 bit
    // все что осталось забираю маской 
    let total_samples = combinated & 0xFFFFFFFFF; // 36 bit

    let steam_info = StreamInfo {
        min_block_size,
        max_block_size,
        min_frame_size,
        max_frame_size,
        sample_rate,
        channels: channels as u8 + 1,
        bps: bps as u8 + 1,
        total_samples,
        checksum_combined,
    };

    println!("{:#?}", steam_info);

    process_metadata(&mut file).unwrap();

    // открытие битового ридера для чтения аудио фреймов из буфера файла
    let mut reader = BitReader::endian(BufReader::new(file), BigEndian);

    
    // чтение синхронизирующего кода из аудио фрейма
    // 15 бит
    // всегда должно быть 0b111111111111100
    let sync_code = reader.read::<15, u16>().expect("Sync error");
    if sync_code != 0x7FFC { panic!("Lost sync"); }

    // 1 бит
    // 0 — фиксированный, 1 — переменный
    // не должен меняться в пределах файла
    let blocking_strategy = reader.read::<1, u8>().unwrap();
    // 4 бита
    // код размера блока
    let block_size_code = reader.read::<4, u8>().unwrap();
    // 4 бита
    // код частоты дискретизации
    let sample_rate = reader.read::<4, u8>().unwrap();
    // 4 бита
    // вариант каналов
    let channel_assignment = reader.read::<4, u8>().unwrap();
    // 3 бита
    // битовая глубина
    let bit_depth = reader.read::<3, u8>().unwrap();
    // 1 бит
    // должен быть 0
    let mandatory = reader.read::<1, u8>().unwrap();
    // 1 байт
    // номер фрейма
    let frame_number = reader.read::<8, u8>().unwrap();
    // 1 байт
    // размер блока
    let block_size = reader.read::<8, u8>().unwrap();
    // 1 байт
    // CRC-8 заголовка фрейма
    let frame_header_crc = reader.read::<8, u8>().unwrap();

    let frame_header = FrameHeader {
        sync_code,
        blocking_strategy,
        block_size_code,
        sample_rate_code: SampleRate::from_u8(sample_rate),
        channel_assignment: ChannelAssignment::from_u8(channel_assignment),
        bit_depth: BitDepth::from_u8(bit_depth),
        mandatory,
        frame_or_sample_number: frame_number as u64,
        block_size,
        crc8: frame_header_crc,
    };

    println!("{:#?}", frame_header);
}
