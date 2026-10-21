use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsciiMode {
    BlackAndWhite,
    Color,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct AsciiOptions {
    pub mode: AsciiMode,
    pub max_columns: u32,
    pub font_size: f32,
    pub character_width: u32,
    pub character_height: u32,
    pub ramp: String,
    pub contrast: i32,
    pub brightness: i32,
    pub invert: bool,
    pub background: String,
    pub max_input_bytes: usize,
    pub max_input_pixels: u64,
    pub max_output_pixels: u64,
}
impl Default for AsciiOptions {
    fn default() -> Self {
        Self {
            mode: AsciiMode::BlackAndWhite,
            max_columns: 100,
            font_size: 16.0,
            character_width: 10,
            character_height: 18,
            ramp: "@#8&o:*. ".to_owned(),
            contrast: 0,
            brightness: 0,
            invert: false,
            background: "#000000".to_owned(),
            max_input_bytes: 10 * 1024 * 1024,
            max_input_pixels: 16_000_000,
            max_output_pixels: 16_000_000,
        }
    }
}
impl AsciiOptions {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_columns == 0 || self.max_columns > 1000 {
            return Err("max_columns must be between 1 and 1000".to_owned());
        }
        if !self.font_size.is_finite() || !(1.0..=256.0).contains(&self.font_size) {
            return Err("font_size must be finite and between 1 and 256".to_owned());
        }
        if self.character_width == 0
            || self.character_height == 0
            || self.character_width > 256
            || self.character_height > 256
        {
            return Err("character dimensions must be between 1 and 256".to_owned());
        }
        if self.ramp.chars().count() < 2 || self.ramp.chars().count() > 256 {
            return Err("ramp must contain between 2 and 256 characters".to_owned());
        }
        if !(-100..=100).contains(&self.contrast) {
            return Err("contrast must be between -100 and 100".to_owned());
        }
        if !(-100..=100).contains(&self.brightness) {
            return Err("brightness must be between -100 and 100".to_owned());
        }
        if parse_background(&self.background).is_none() {
            return Err("background must be a six-digit hexadecimal color".to_owned());
        }
        if self.max_input_bytes == 0 || self.max_input_pixels == 0 || self.max_output_pixels == 0 {
            return Err("input and output limits must be greater than zero".to_owned());
        }
        Ok(())
    }
}

pub(crate) fn parse_background(value: &str) -> Option<[u8; 4]> {
    let value = value.strip_prefix('#')?;
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some([
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
        u8::MAX,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_rejects_unicode_and_malformed_hex_without_panicking() {
        for value in ["#a€bc", "#💥ab", "#１２", "#gg0000", "ffffff", "#fff"] {
            assert_eq!(parse_background(value), None, "{value}");
        }
        assert_eq!(parse_background("#aB09fF"), Some([171, 9, 255, 255]));
    }

    #[test]
    fn font_size_must_be_finite_and_bounded() {
        for font_size in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, 257.0] {
            assert!(AsciiOptions {
                font_size,
                ..AsciiOptions::default()
            }
            .validate()
            .is_err());
        }
        for font_size in [1.0, 16.0, 256.0] {
            assert!(AsciiOptions {
                font_size,
                ..AsciiOptions::default()
            }
            .validate()
            .is_ok());
        }
    }

    #[test]
    fn output_budget_must_be_positive() {
        assert!(AsciiOptions {
            max_output_pixels: 0,
            ..AsciiOptions::default()
        }
        .validate()
        .is_err());
    }
}
