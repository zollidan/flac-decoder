use std::fs::File;
use std::io::{Read};


// docs : https://www.rfc-editor.org/rfc/rfc9639.html#name-examples
fn main() {
    let path = "song.flac";

    let mut file = File::open(path).unwrap();

    let mut format_part = [0u8; 4];
    
    file.read_exact(&mut format_part);

    if &format_part != b"fLaC" {
        panic!("This is not a FLAC file.");
    }

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

    println!("Is last: {}, Block type: {}, Length: {}", is_last, block_type, length);

    if block_type != 0 {
        panic!("Expect STREAMINFO (type 0)");
    }

    // создаю вектор в длину блока и читаю его содержимое
    let mut streaminfo = vec![0u8; length as usize];
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
    let total_samples = combinated & 0xFFFFFFFFF; // 36 bit1

    println!("Min block size: {}", min_block_size);
    println!("Max block size: {}", max_block_size);
    println!("Min frame size: {}", min_frame_size);
    println!("Max frame size: {}", max_frame_size);
    println!("Sample rate: {}", sample_rate);
    println!("Channels: {}", channels + 1);
    println!("Bits per sample: {}", bps + 1);
    println!("Total samples: {}", total_samples);
    println!("MD5 checksum: {:x?}", checksum_combined);

}