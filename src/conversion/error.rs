use thiserror::Error;
#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("input is too large: {actual} bytes exceeds the {max} byte limit")]
    InputTooLarge { actual: usize, max: usize },
    #[error("decoded image is too large: {actual} pixels exceeds the {max} pixel limit")]
    ImageTooLarge { actual: u64, max: u64 },
    #[error("image decoding failed: {0}")]
    Image(#[from] image::ImageError),
    #[error("a font is required to render ASCII output")]
    MissingFont,
    #[error("font data is invalid")]
    InvalidFont,
    #[error("invalid conversion options: {0}")]
    InvalidOptions(String),
}
