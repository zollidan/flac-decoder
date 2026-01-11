use std::io::{self, Read, Seek, SeekFrom};

use image::ImageReader;
use std::io::Cursor;

#[derive(Debug)]
pub struct PictureBlock {
    pub picture_type: u32,
    pub media_type: String,
    pub description_length: u32,
    pub width: u32,
    pub height: u32,
    pub color_depth: u32,
    pub colors_used: u32,
    pub picture_data_length: u32,
}

impl PictureBlock {
    // получение и сохранение картинки из метаданных
    pub fn process_picture_block(picture_block: Vec<u8>, save_cover: bool) {
        let mut step = 0;

        let picture_type = u32::from_be_bytes(picture_block[step..step + 4].try_into().unwrap());
        step += 4;

        let media_type_length =
            u32::from_be_bytes(picture_block[step..step + 4].try_into().unwrap());
        step += 4;

        let media_type =
            std::str::from_utf8(&picture_block[step..step + media_type_length as usize]).unwrap();
        step += media_type_length as usize;

        let description_length =
            u32::from_be_bytes(picture_block[step..step + 4].try_into().unwrap());
        step += 4;
        step += description_length as usize;

        let mut width = u32::from_be_bytes(picture_block[step..step + 4].try_into().unwrap());
        step += 4;
        let mut height = u32::from_be_bytes(picture_block[step..step + 4].try_into().unwrap());
        step += 4;
        let color_depth = u32::from_be_bytes(picture_block[step..step + 4].try_into().unwrap());
        step += 4;
        let colors_used = u32::from_be_bytes(picture_block[step..step + 4].try_into().unwrap());
        step += 4;
        let picture_data_length =
            u32::from_be_bytes(picture_block[step..step + 4].try_into().unwrap());
        step += 4;
        let picture_data = &picture_block[step..step + picture_data_length as usize];

        // сохранение картинки в файл только если указан флаг
        if save_cover {
            let file_name = format!(
                "picture_{}.{}",
                picture_type,
                match media_type {
                    "image/jpeg" => "jpg",
                    "image/png" => "png",
                    _ => "bin",
                }
            );

            let cursor = Cursor::new(picture_data);

            match ImageReader::new(cursor).with_guessed_format() {
                Ok(reader) => match reader.decode() {
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
                },
                Err(e) => {
                    println!("Failed to read image dimensions: {}", e);
                }
            }
        }

        Self {
            picture_type,
            media_type: media_type.to_string(),
            description_length,
            width,
            height,
            color_depth,
            colors_used,
            picture_data_length,
        };
    }
}

pub fn get_header<R: Read>(reader: &mut R) -> Result<(bool, u8, u32), std::io::Error> {
    let mut header = [0u8; 4];
    reader.read_exact(&mut header)?;

    // побитовая операция
    // первый бит 0 или 1 если 0 то это не последний блок метаданных
    // следующие 7 бит - тип блока 0 - STREAMINFO 1 - PADDING и тд
    let is_last = (header[0] & 0x80) != 0;
    let block_type = header[0] & 0x7F;

    // следующие 3 байта - длина блока метаданных
    // собираю 24 бита из 3 байт
    // сдвигаю первый байт на 16 бит влево, второй на 8 бит и добавляю третий
    let length = ((header[1] as u32) << 16) | ((header[2] as u32) << 8) | (header[3] as u32);

    Ok((is_last, block_type, length))
}

pub fn process_metadata<R: Read + Seek>(reader: &mut R, save_cover: bool) -> io::Result<()> {
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
        let (is_last, block_type, length) = get_header(reader)?;

        // пока работает только обработка блока картинки
        match block_type {
            // блок картинки
            6 => {
                let mut buffer = vec![0u8; length as usize];
                reader.read_exact(&mut buffer)?;
                PictureBlock::process_picture_block(buffer, save_cover);
            }
            _ => {
                // пропускаем остальные блоки
                reader.seek(SeekFrom::Current(length as i64))?;
            }
        }

        if is_last {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_header() {
        let data = vec![0b10000110, 0x00, 0x00, 0x1A]; // is_last = true, block_type = 6, length = 26
        let mut cursor = Cursor::new(data);

        let (is_last, block_type, length) = get_header(&mut cursor).unwrap();

        assert!(is_last);
        assert_eq!(block_type, 6);
        assert_eq!(length, 26);
    }

    #[test]
    fn test_get_header_invalid() {
        let data = vec![0b10000110, 0x00]; // мало данных
        let mut cursor = Cursor::new(data);

        let result = get_header(&mut cursor);

        assert!(result.is_err());
        assert_eq!(result.err().unwrap().kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn test_process_metadata_no_blocks() {
        let data = vec![];
        let mut cursor = Cursor::new(data);
        let result = process_metadata(&mut cursor, false);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().kind(), io::ErrorKind::UnexpectedEof);
    }

    // #[test]
    // fn test_process_metadata_skips_and_finishes() {
    //     let mut data = Vec::new();

    //     // блок Padding: is_last=false, type=1, length=4
    //     // [0x01, 0x00, 0x00, 0x04]
    //     data.extend_from_slice(&[0x01, 0x00, 0x00, 0x04]);
    //     data.extend_from_slice(&[0xAA, 0xAA, 0xAA, 0xAA]);

    //     // блок Picture: is_last=true, type=6, length=2
    //     data.extend_from_slice(&[0x86, 0x00, 0x00, 0x02]);
    //     data.extend_from_slice(&[0xFF, 0xFF]);

    //     let mut cursor = Cursor::new(data);

    //     let result = process_metadata(&mut cursor, false);

    //     assert!(result.is_ok());
    //     assert_eq!(cursor.position(), 12);
    // }
}
