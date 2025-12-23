use std::io::Cursor;

use image::ImageReader;


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
    pub fn process_picture_block(picture_block: Vec<u8>) {
        let mut step = 0;

        let picture_type = u32::from_be_bytes(picture_block[step..step + 4].try_into().unwrap());
        step += 4;

        let media_type_length = u32::from_be_bytes(picture_block[step..step + 4].try_into().unwrap());
        step += 4;

        let media_type =
            std::str::from_utf8(&picture_block[step..step + media_type_length as usize]).unwrap();
        step += media_type_length as usize;

        let description_length = u32::from_be_bytes(picture_block[step..step + 4].try_into().unwrap());
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
        let picture_data_length = u32::from_be_bytes(picture_block[step..step + 4].try_into().unwrap());
        step += 4;
        let picture_data = &picture_block[step..step + picture_data_length as usize];

        // сохранение картинки в файл
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

        let picture = PictureBlock {
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
}

