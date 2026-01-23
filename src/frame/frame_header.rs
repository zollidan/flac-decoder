use bitstream_io::{BigEndian, BitRead, BitReader};
use std::{
    fs::File,
    io::{BufReader, Error, ErrorKind, Read},
};

use crate::metadata::stream_info;

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

impl FrameHeader {
    pub fn read_frame_header(
        reader: &mut BitReader<BufReader<File>, BigEndian>,
        stream_info: &stream_info::StreamInfo,
    ) -> Result<Self, std::io::Error> {
        // чтение синхронизирующего кода из аудио фрейма
        // 14 бит (не 15!)
        // всегда должно быть 0b11111111111110
        let sync_code = reader.read::<14, u16>()?;

        if sync_code != 0x3FFE {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Invalid sync code: {:b}, expected 11111111111110",
                    sync_code
                ),
            ));
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
            0b0000 => stream_info.sample_rate as f32 / 1000.0, // взять из streaminfo
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
            0b000 => stream_info.bps as u32, // взять из streaminfo
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
        let frame_or_sample_number = read_utf8_u64(reader).unwrap();

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

        Ok(Self {
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
        })
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_utf8_u64_table() {
        // Таблица: (входные байты, ожидаемый результат или ошибка)
        let cases = vec![
            (vec![0x00], Ok(0)),                                   // минимально 1 байт
            (vec![0x7F], Ok(127)),                                 // максимально 1 байт
            (vec![0xC2, 0x80], Ok(128)),                           // минимально 2 байта
            (vec![0xE2, 0x82, 0xAC], Ok(0x20AC)),                  // символы евро (3 байта)
            (vec![0b10000000], Err("Invalid UTF-8 sequence")),     // начинается с 10...
            (vec![0xC2, 0x41], Err("Invalid UTF-8 continuation")), // бита продолжения нет (0x41 = 'A')
        ];

        for (data, expected) in cases {
            let mut reader = BitReader::endian(&data[..], BigEndian);
            let result = read_utf8_u64(&mut reader);

            match expected {
                Ok(expected_val) => {
                    assert_eq!(
                        result.unwrap(),
                        expected_val,
                        "Error input data: {:?}",
                        data
                    );
                }
                Err(err_msg) => {
                    let err = result.unwrap_err();
                    assert_eq!(
                        err.to_string(),
                        err_msg,
                        "Expected error '{}', but data {:?} passed",
                        err_msg,
                        data
                    );
                }
            }
        }
    }
}
