use std::{fs::File, io::{self, Read, Seek, SeekFrom}};


pub fn get_header(file: &mut File) -> Result<(bool, u8, u32), std::io::Error> {
    let mut header = [0u8; 4];
    file.read_exact(&mut header)?;

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

pub fn process_metadata(file: &mut File) -> io::Result<()> {
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
        let (is_last, block_type, length) = get_header(file)?;

        // пока работает только обработка блока картинки
        match block_type {
            // блок картинки
            6 => {
                let mut buffer = vec![0u8; length as usize];
                file.read_exact(&mut buffer)?;
                picture::PictureBlock::process_picture_block(buffer);
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