use super::{options::parse_background, AsciiMode, AsciiOptions, ConversionError};
use ab_glyph::{FontArc, PxScale};
use image::imageops::FilterType;
use image::{DynamicImage, GrayImage, ImageFormat, ImageReader, Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;
use std::io::Cursor;
pub fn convert_image_to_png(
    input: &[u8],
    font_data: &[u8],
    options: &AsciiOptions,
) -> Result<Vec<u8>, ConversionError> {
    options
        .validate()
        .map_err(ConversionError::InvalidOptions)?;
    if input.len() > options.max_input_bytes {
        return Err(ConversionError::InputTooLarge {
            actual: input.len(),
            max: options.max_input_bytes,
        });
    }
    if font_data.is_empty() {
        return Err(ConversionError::MissingFont);
    }
    let image = ImageReader::new(Cursor::new(input))
        .with_guessed_format()
        .map_err(image::ImageError::IoError)?
        .decode()?;
    let pixels = u64::from(image.width()) * u64::from(image.height());
    if pixels > options.max_input_pixels {
        return Err(ConversionError::ImageTooLarge {
            actual: pixels,
            max: options.max_input_pixels,
        });
    }
    let rendered = render_ascii(&image, font_data, options)?;
    let mut output = Cursor::new(Vec::new());
    rendered.write_to(&mut output, ImageFormat::Png)?;
    Ok(output.into_inner())
}

fn render_ascii(
    image: &DynamicImage,
    font_data: &[u8],
    options: &AsciiOptions,
) -> Result<RgbaImage, ConversionError> {
    let source = image.to_rgba8();
    let columns = source.width().min(options.max_columns).max(1);
    let rows = ((source.height() as f32 * columns as f32 / source.width() as f32) * 0.5)
        .round()
        .max(1.0) as u32;
    let resized = image::imageops::resize(&source, columns, rows, FilterType::Triangle);
    let grayscale = DynamicImage::ImageRgba8(resized.clone()).to_luma8();
    let (black_point, white_point) = contrast_bounds(&grayscale);
    let font =
        FontArc::try_from_vec(font_data.to_vec()).map_err(|_| ConversionError::InvalidFont)?;
    let ramp: Vec<char> = options.ramp.chars().collect();
    let background =
        parse_background(&options.background).ok_or(ConversionError::InvalidOptions(
            "background must be a six-digit hexadecimal color".to_owned(),
        ))?;
    let mut output = RgbaImage::from_pixel(
        columns * options.character_width,
        rows * options.character_height,
        Rgba(background),
    );
    for y in 0..rows {
        for x in 0..columns {
            let pixel = resized.get_pixel(x, y);
            let value = adjust_luminance(
                normalize_luminance(grayscale.get_pixel(x, y)[0], black_point, white_point),
                options,
            );
            let index = (value as usize * (ramp.len() - 1)) / u8::MAX as usize;
            let character = ramp[index];
            let color = match options.mode {
                AsciiMode::BlackAndWhite => Rgba([u8::MAX; 4]),
                AsciiMode::Color => Rgba([pixel[0], pixel[1], pixel[2], u8::MAX]),
            };
            draw_text_mut(
                &mut output,
                color,
                (x * options.character_width) as i32,
                (y * options.character_height) as i32,
                PxScale::from(options.font_size),
                &font,
                &character.to_string(),
            );
        }
    }
    Ok(output)
}

fn contrast_bounds(image: &GrayImage) -> (u8, u8) {
    let mut values: Vec<u8> = image.pixels().map(|pixel| pixel[0]).collect();
    values.sort_unstable();
    let last = values.len().saturating_sub(1);
    let black = values[last * 2 / 100];
    let white = values[last * 98 / 100].max(black.saturating_add(1));
    (black, white)
}
fn normalize_luminance(value: u8, black: u8, white: u8) -> u8 {
    if value <= black {
        return 0;
    }
    if value >= white {
        return u8::MAX;
    }
    ((value - black) as u16 * u8::MAX as u16 / (white - black) as u16) as u8
}

fn adjust_luminance(value: u8, options: &AsciiOptions) -> u8 {
    let centered = value as f32 / 255.0;
    let contrast = (options.contrast + 100) as f32 / 100.0;
    let brightness = options.brightness as f32 / 100.0;
    let adjusted = ((centered - 0.5) * contrast + 0.5 + brightness).clamp(0.0, 1.0);
    let adjusted = if options.invert {
        1.0 - adjusted
    } else {
        adjusted
    };
    (adjusted * 255.0).round() as u8
}
