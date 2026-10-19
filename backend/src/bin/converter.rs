use ascii_image_converter::{convert_image_to_png, AsciiMode, AsciiOptions};
use std::env;
use std::error::Error;
use std::fs;
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let input_path = args.next().unwrap_or_else(|| "input_image.png".to_owned());
    let output_path = args.next().unwrap_or_else(|| "output_image.png".to_owned());
    let mode = match env::var("ASCII_MODE").as_deref() {
        Ok("color") => AsciiMode::Color,
        Ok("black_and_white") | Ok("bw") | Err(_) => AsciiMode::BlackAndWhite,
        Ok(value) => return Err(format!("unsupported ASCII_MODE: {value}").into()),
    };
    let font_path = env::var("ASCII_FONT_PATH")
        .map_err(|_| "ASCII_FONT_PATH must point to a .ttf font file")?;
    let input = fs::read(input_path)?;
    let font = fs::read(font_path)?;
    let output = convert_image_to_png(
        &input,
        &font,
        &AsciiOptions {
            mode,
            ..AsciiOptions::default()
        },
    )?;
    fs::write(output_path, output)?;
    println!("ASCII image created successfully");
    Ok(())
}
