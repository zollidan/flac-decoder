#![warn(clippy::all, clippy::pedantic)]

use std::fs::File;
use std::io::{self, Cursor, Read};

use image::ImageReader;


// docs : https://www.rfc-editor.org/rfc/rfc9639.html#name-examples

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
    println!("Total picture block size: {}", picture_block.len());
    println!("First 32 bytes: {:?}", &picture_block[0..32.min(picture_block.len())]);

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
    
    println!("Picture type: {}", picture_type);
    println!("Media type: {:?}", media_type);
    println!("Description length: {}", description_length);
    println!("Width: {}", width);
    println!("Height: {}", height);
    println!("Color depth: {}", color_depth);
    println!("Colors used: {}", colors_used);
    println!("Picture data length: {}", picture_data_length);


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
    let min_block_size = ((streaminfo[0] as u16) << 8) | (streaminfo[1] as u16);
    let max_block_size = ((streaminfo[2] as u16) << 8) | (streaminfo[3] as u16);
    let min_frame_size = (streaminfo[4] as u32) << 16 | (streaminfo[5] as u32) << 8 | (streaminfo[6] as u32);
    let max_frame_size = (streaminfo[7] as u32) << 16 | (streaminfo[8] as u32) << 8 | (streaminfo[9] as u32);
    // беру сразу 8 байт с 10 по 17 и комбинирую в одно 64 битное число
    // так как дальше идут значения которые занимают биты в этих байтах
    // так удобнее всего двигаться внутри байтов 
    let combinated = (streaminfo[10] as u64) << 56 | 
                            (streaminfo[11] as u64) << 48 | 
                            (streaminfo[12] as u64) << 40 | 
                            (streaminfo[13] as u64) << 32 |
                            (streaminfo[14] as u64) << 24 |
                            (streaminfo[15] as u64) << 16 |
                            (streaminfo[16] as u64) << 8 |
                            (streaminfo[17] as u64);
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

    println!("Min block size: {}", min_block_size);
    println!("Max block size: {}", max_block_size);
    println!("Min frame size: {}", min_frame_size);
    println!("Max frame size: {}", max_frame_size);
    println!("Sample rate: {}", sample_rate);
    println!("Channels: {}", channels + 1);
    println!("Bits per sample: {}", bps + 1);
    println!("Total samples: {}", total_samples);
    println!("MD5 checksum: {:x?}", checksum_combined);

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
        let (is_last, block_type, length) = get_header(&mut file);
        println!("is_last: {}, Block type: {}, length: {}", is_last, block_type, length);
        let mut buffer = vec![0u8; length as usize];
        file.read_exact(&mut buffer).unwrap();

        if block_type == 6 {
            // обработка блока картинки
            process_picture_block(buffer);
        }

        if is_last {
            break;
        }
    }

}
