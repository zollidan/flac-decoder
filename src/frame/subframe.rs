use bitstream_io::{BigEndian, BitRead, BitReader};
use std::fs::File;
use std::io::BufReader;

struct SubframeHeader {}

pub struct Subframe {
    subframe_header: SubframeHeader,
}

// функция для поиска количества битов, отведенных под убитые биты
// хорошо бы потом сделать -> Result<u32, std::io::Error>
pub fn find_wasted_bits(reader: &mut BitReader<BufReader<File>, BigEndian>) -> u32 {
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

pub fn constant_value() {}

pub fn verbatim() {}

pub fn fixed_prediction(
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

pub fn decode_rice_residual(
    reader: &mut BitReader<BufReader<File>, BigEndian>,
    order: u8,
    block_size: u32,
) -> Vec<i64> {
    // заглушка
    vec![]
}

pub fn lpc() {}
