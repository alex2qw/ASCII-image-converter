#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsciiMode {
    BlackAndWhite,
    Color,
}
#[derive(Clone, Debug)]
pub struct AsciiOptions {
    pub mode: AsciiMode,
    pub max_columns: u32,
    pub font_size: f32,
    pub character_width: u32,
    pub character_height: u32,
    pub max_input_bytes: usize,
    pub max_input_pixels: u64,
}
impl Default for AsciiOptions {
    fn default() -> Self {
        Self {
            mode: AsciiMode::BlackAndWhite,
            max_columns: 100,
            font_size: 16.0,
            character_width: 10,
            character_height: 18,
            max_input_bytes: 10 * 1024 * 1024,
            max_input_pixels: 16_000_000,
        }
    }
}
impl AsciiOptions {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_columns == 0 || self.max_columns > 1000 {
            return Err("max_columns must be between 1 and 1000".to_owned());
        }
        if self.font_size <= 0.0 {
            return Err("font_size must be greater than zero".to_owned());
        }
        if self.character_width == 0 || self.character_height == 0 || self.character_width > 256 || self.character_height > 256 {
            return Err("character dimensions must be between 1 and 256".to_owned());
        }
        if self.max_input_bytes == 0 || self.max_input_pixels == 0 {
            return Err("input limits must be greater than zero".to_owned());
        }
        Ok(())
    }
}
