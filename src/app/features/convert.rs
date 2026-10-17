use std::error::Error;
use std::fs;
use std::path::Path;

use ab_glyph::{FontArc, PxScale};
use image::imageops::FilterType;
use image::{GrayImage, ImageReader, Luma};
use imageproc::drawing::draw_text_mut;

const CHARACTER_RAMP: &[u8] = b"@%#*+=-:. ";
const MAX_COLUMNS: u32 = 160;
const FONT_PATH: &str = "/System/Library/Fonts/Supplemental/Courier New.ttf";
const FONT_SIZE: f32 = 14.0;
const CHARACTER_WIDTH: u32 = 8;
const CHARACTER_HEIGHT: u32 = 16;

fn luminance_to_character(luminance: u8) -> char {
    let ramp_index =
        (luminance as usize * (CHARACTER_RAMP.len() - 1)) / u8::MAX as usize;
    CHARACTER_RAMP[ramp_index] as char
}

fn convert_to_ascii_image(image: &image::DynamicImage) -> Result<GrayImage, Box<dyn Error>> {
    let source = image.to_luma8();
    let columns = source.width().min(MAX_COLUMNS).max(1);
    let rows = ((source.height() as f32 * columns as f32 / source.width() as f32) * 0.5)
        .round()
        .max(1.0) as u32;
    let resized = image::imageops::resize(&source, columns, rows, FilterType::Triangle);
    let mut output = GrayImage::from_pixel(
        columns * CHARACTER_WIDTH,
        rows * CHARACTER_HEIGHT,
        Luma([0]),
    );
    let font = FontArc::try_from_vec(fs::read(FONT_PATH)?)
        .map_err(|_| format!("could not load font at {FONT_PATH}"))?;
    let scale = PxScale::from(FONT_SIZE);

    for (x, y, pixel) in resized.enumerate_pixels() {
        let character = luminance_to_character(pixel[0]);
        draw_text_mut(
            &mut output,
            Luma([u8::MAX]),
            (x * CHARACTER_WIDTH) as i32,
            (y * CHARACTER_HEIGHT) as i32,
            scale,
            &font,
            &character.to_string(),
        );
    }

    Ok(output)
}

fn load_image(path: impl AsRef<Path>) -> image::ImageResult<image::DynamicImage> {
    ImageReader::open(path)?.decode()
}

fn convert_image_to_ascii(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> Result<(), Box<dyn Error>> {
    let image = load_image(input_path)?;
    let ascii_image = convert_to_ascii_image(&image)?;
    ascii_image.save(output_path)?;
    Ok(())
}

fn main() {
    let input_path = "input_image.png";
    let output_path = "output_image.png";

    match convert_image_to_ascii(input_path, output_path) {
        Ok(()) => println!("Image converted to an ASCII image successfully!"),
        Err(error) => eprintln!("Error converting image: {error}"),
    }
}
