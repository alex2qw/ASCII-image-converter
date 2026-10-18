use serde::ser::SerializeStruct;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("input is too large: {actual} bytes exceeds the {max} byte limit")]
    InputTooLarge { actual: usize, max: usize },
    #[error("decoded image is too large: {actual} pixels exceeds the {max} pixel limit")]
    ImageTooLarge { actual: u64, max: u64 },
    #[error("request is too large for the configured upload limit")]
    RequestTooLarge,
    #[error("image decoding failed: {0}")]
    Image(#[from] image::ImageError),
    #[error("a font is required to render ASCII output")]
    MissingFont,
    #[error("font data is invalid")]
    InvalidFont,
    #[error("invalid conversion options: {0}")]
    InvalidOptions(String),
}

impl Serialize for ConversionError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut value = serializer.serialize_struct("ConversionError", 2)?;
        value.serialize_field("code", self.code())?;
        value.serialize_field("message", &self.to_string())?;
        value.end()
    }
}

impl ConversionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InputTooLarge { .. } => "input_too_large",
            Self::ImageTooLarge { .. } => "image_too_large",
            Self::RequestTooLarge => "request_too_large",
            Self::Image(_) => "invalid_image",
            Self::MissingFont => "missing_font",
            Self::InvalidFont => "invalid_font",
            Self::InvalidOptions(_) => "invalid_options",
        }
    }
}
