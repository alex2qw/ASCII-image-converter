use super::{options::parse_background, AsciiMode, AsciiOptions, ConversionError};
use ab_glyph::{FontArc, PxScale};
use image::imageops::FilterType;
use image::{
    DynamicImage, GrayImage, ImageDecoder, ImageFormat, ImageReader, Limits, Rgba, RgbaImage,
};
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
    let mut reader = ImageReader::new(Cursor::new(input))
        .with_guessed_format()
        .map_err(image::ImageError::IoError)?;
    // Decoder allocation limits are best-effort for some formats. The explicit
    // dimension checks below happen before decoding the full pixel buffer.
    let mut limits = Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    let pixels = u64::from(width) * u64::from(height);
    if pixels > options.max_input_pixels {
        return Err(ConversionError::ImageTooLarge {
            actual: pixels,
            max: options.max_input_pixels,
        });
    }
    // Reject excessive output before spending time and memory on pixel decoding.
    let dimensions = render_dimensions(width, height, options)?;
    let image = DynamicImage::from_decoder(decoder)?;
    let rendered = render_ascii(&image, font_data, options, dimensions)?;
    let mut output = Cursor::new(Vec::new());
    rendered.write_to(&mut output, ImageFormat::Png)?;
    Ok(output.into_inner())
}

// Integer arithmetic avoids float rounding and checks sizes before narrowing.
fn render_dimensions(
    width: u32,
    height: u32,
    options: &AsciiOptions,
) -> Result<(u32, u32, u32, u32), ConversionError> {
    if width == 0 || height == 0 {
        return Err(ConversionError::InvalidOptions(
            "image dimensions must be nonzero".to_owned(),
        ));
    }
    let columns = width.min(options.max_columns);
    let rows = ((u64::from(height) * u64::from(columns) + u64::from(width))
        / (2 * u64::from(width)))
    .max(1);
    let output_width = u64::from(columns) * u64::from(options.character_width);
    let output_height = rows * u64::from(options.character_height);
    let pixels = output_width.saturating_mul(output_height);
    // imageproc uses signed coordinates; the RGBA buffer must also fit usize.
    if pixels > options.max_output_pixels
        || output_width > i32::MAX as u64
        || output_height > i32::MAX as u64
        || pixels > (usize::MAX / 4) as u64
    {
        return Err(ConversionError::OutputTooLarge {
            actual: pixels,
            max: options.max_output_pixels,
        });
    }
    Ok((
        columns,
        rows as u32,
        output_width as u32,
        output_height as u32,
    ))
}

fn render_ascii(
    image: &DynamicImage,
    font_data: &[u8],
    options: &AsciiOptions,
    dimensions: (u32, u32, u32, u32),
) -> Result<RgbaImage, ConversionError> {
    let (columns, rows, output_width, output_height) = dimensions;
    // Resize before converting to RGBA to avoid cloning the full source buffer.
    let resized = image
        .resize_exact(columns, rows, FilterType::Triangle)
        .to_rgba8();
    let grayscale = DynamicImage::ImageRgba8(resized.clone()).to_luma8();
    let (black_point, white_point) = contrast_bounds(&grayscale);
    let font =
        FontArc::try_from_vec(font_data.to_vec()).map_err(|_| ConversionError::InvalidFont)?;
    let ramp: Vec<char> = options.ramp.chars().collect();
    let background =
        parse_background(&options.background).ok_or(ConversionError::InvalidOptions(
            "background must be a six-digit hexadecimal color".to_owned(),
        ))?;
    let mut output = RgbaImage::from_pixel(output_width, output_height, Rgba(background));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_preserve_normal_rendering_and_allow_exact_budget() {
        let options = AsciiOptions {
            max_output_pixels: 900_000,
            ..AsciiOptions::default()
        };
        assert_eq!(
            render_dimensions(200, 200, &options).unwrap(),
            (100, 50, 1000, 900)
        );
        let options = AsciiOptions {
            max_output_pixels: 899_999,
            ..options
        };
        assert!(matches!(
            render_dimensions(200, 200, &options),
            Err(ConversionError::OutputTooLarge { .. })
        ));
    }

    #[test]
    fn tall_images_and_extreme_cells_are_rejected() {
        let options = AsciiOptions::default();
        assert!(matches!(
            render_dimensions(1, 16_000_000, &options),
            Err(ConversionError::OutputTooLarge { .. })
        ));
        let options = AsciiOptions {
            character_width: 256,
            character_height: 256,
            ..options
        };
        assert!(matches!(
            render_dimensions(1000, 1000, &options),
            Err(ConversionError::OutputTooLarge { .. })
        ));
    }

    #[test]
    fn oversized_input_is_rejected_before_pixel_decoding() {
        // A valid BMP header advertising 100x100 pixels, with no pixel payload.
        // Decoding would fail; the expected size error proves we reject earlier.
        let mut bmp = vec![0u8; 54];
        bmp[0..2].copy_from_slice(b"BM");
        bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&100u32.to_le_bytes());
        bmp[22..26].copy_from_slice(&100u32.to_le_bytes());
        bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&24u16.to_le_bytes());
        let options = AsciiOptions {
            max_input_pixels: 9999,
            ..AsciiOptions::default()
        };
        assert!(matches!(
            convert_image_to_png(&bmp, &[1], &options),
            Err(ConversionError::ImageTooLarge {
                actual: 10_000,
                max: 9999
            })
        ));
        let options = AsciiOptions {
            max_output_pixels: 1,
            ..AsciiOptions::default()
        };
        assert!(matches!(
            convert_image_to_png(&bmp, &[1], &options),
            Err(ConversionError::OutputTooLarge { .. })
        ));
    }
}
