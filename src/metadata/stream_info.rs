use super::blocks::get_header;
use std::{
    fs::File,
    io::{self, Read},
};

#[derive(Debug)]
pub struct StreamInfo {
    pub min_block_size: u16,
    pub max_block_size: u16,
    pub min_frame_size: u32,
    pub max_frame_size: u32,
    pub sample_rate: u64,
    pub channels: u8,
    pub bps: u8,
    pub total_samples: u64,
    pub checksum_combined: [u8; 16],
}

impl StreamInfo {
    pub fn new(
        min_block_size: u16,
        max_block_size: u16,
        min_frame_size: u32,
        max_frame_size: u32,
        sample_rate: u64,
        channels: u8,
        bps: u8,
        total_samples: u64,
        checksum_combined: [u8; 16],
    ) -> Self {
        StreamInfo {
            min_block_size,
            max_block_size,
            min_frame_size,
            max_frame_size,
            sample_rate,
            channels,
            bps,
            total_samples,
            checksum_combined,
        }
    }

    pub fn process_stream_info_block(file: &mut File) -> StreamInfo {
        let streaminfo_header = get_header(file).expect("Error get_header!");

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
        let channels = (combinated >> 41) & 0x7; // 3 bit
        // сдвигаю от 32 на 4 бит и маской беру 5 бит
        let bps = (combinated >> 36) & 0x1F; // 5 bit
        // все что осталось забираю маской
        let total_samples = combinated & 0xFFFFFFFFF; // 36 bit

        let steam_info = StreamInfo::new(
            min_block_size,
            max_block_size,
            min_frame_size,
            max_frame_size,
            sample_rate,
            channels as u8,
            bps as u8,
            total_samples,
            checksum_combined,
        );

        println!("{:#?}", steam_info);

        steam_info
    }
}

pub fn check_flac_header(file: &mut File) -> io::Result<()> {
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
