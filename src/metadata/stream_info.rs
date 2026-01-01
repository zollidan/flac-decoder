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

    pub fn process_stream_info_block<R: Read>(reader: &mut R) -> StreamInfo {
        let streaminfo_header = get_header(reader).expect("Error get_header!");

        // первый всегда идет STREAMINFO
        // поменять потом с индексов на именованные поля
        if streaminfo_header.1 != 0 {
            panic!("Expect STREAMINFO (type 0)");
        }

        // создаю вектор в длину блока и читаю его содержимое
        let mut streaminfo = vec![0u8; streaminfo_header.2 as usize];
        reader.read_exact(&mut streaminfo).unwrap();

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

        Self {
            min_block_size,
            max_block_size,
            min_frame_size,
            max_frame_size,
            sample_rate,
            channels: (channels + 1) as u8,
            bps: (bps + 1) as u8,        
            total_samples,
            checksum_combined,
        }
    }
}

pub fn check_flac_header<R: Read>(reader: &mut R) -> io::Result<()> {
    let mut format_part = [0u8; 4];
    reader.read_exact(&mut format_part)?;
    if &format_part != b"fLaC" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Not a FLAC file",
        ));
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    #[should_panic(expected = "Expect STREAMINFO (type 0)")] // должна случиться паника
    fn test_process_stream_info_block_invalid_header() {
        // данные -> [неправильный тип padding 1, длина 0, 0, 1]
        let mut data = Cursor::new(vec![
            0x01, 0x00, 0x00, 0x01, 0x00
        ]);
        
        StreamInfo::process_stream_info_block(&mut data);
    }

    #[test]
    fn test_process_stream_info_block_full_check() {
        let mut data = Vec::new();

        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x22]);
        data.extend_from_slice(&[0x10, 0x00]);
        data.extend_from_slice(&[0x10, 0x00]);
        data.extend_from_slice(&[0x00, 0x00, 0x01]);
        data.extend_from_slice(&[0x00, 0x00, 0x02]);

        let sample_rate: u64 = 44100;
        let channels: u64 = 2;
        let bps: u64 = 15; 
        let total_samples: u64 = 1000;

        let combined: u64 = (sample_rate << 44) 
                        | (channels << 41) 
                        | (bps << 36) 
                        | total_samples;
        
        data.extend_from_slice(&combined.to_be_bytes());

        // MD5 (16 байт)
        let md5 = [0xAB; 16];
        data.extend_from_slice(&md5);

        // проверка
        let mut reader = Cursor::new(data);
        let info = StreamInfo::process_stream_info_block(&mut reader);

    }

    #[test]
    fn test_check_flac_header_valid() {
        let mut data = Cursor::new("fLaCotherdataidkrandom");
        let result = check_flac_header(&mut data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_flac_header_invalid() {
        let mut data = Cursor::new("NotFLACdatahere");
        let result = check_flac_header(&mut data);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_check_flac_header_too_short() {
        let mut data = Cursor::new("fLa");
        let result = check_flac_header(&mut data);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
    }
}