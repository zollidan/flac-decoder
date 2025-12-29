use std::io::Read;
use bitstream_io::{BigEndian, BitRead, BitReader};

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

// функция для чтения переменной длины UTF-8 закодированного u64
pub fn read_utf8_u64<R: Read>(reader: &mut BitReader<R, BigEndian>) -> std::io::Result<u64> {
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
