# ASCII image converter

Rust library and CLI for converting images into rendered ASCII PNG images.

## Library API

The web layer should pass uploaded image bytes and font bytes directly to the library:

~~~rust
use ascii_image_converter::{convert_image_to_png, AsciiMode, AsciiOptions};

let options = AsciiOptions {
    mode: AsciiMode::Color,
    ..AsciiOptions::default()
};
let output_png = convert_image_to_png(&image_bytes, &font_bytes, &options)?;
~~~

The function returns PNG bytes suitable for an HTTP response. It enforces input-byte and decoded-pixel limits before rendering. Configure those limits through AsciiOptions for the web application policy.

The font is supplied by the caller, so the library has no operating-system-specific font path. A web application can load a bundled, licensed font during startup and reuse its bytes for requests.

## CLI

The CLI reads a local input file and writes a local output file. It requires a TrueType font path through ASCII_FONT_PATH:

~~~sh
ASCII_FONT_PATH=/path/to/monospace.ttf cargo run -- input_image.png output_image.png
ASCII_FONT_PATH=/path/to/monospace.ttf ASCII_MODE=color cargo run -- input_image.png output_image.png
~~~

Supported modes are black_and_white (the default), bw, and color.

## Project layout

~~~text
src/lib.rs                    Public library API
src/conversion/options.rs     Modes, rendering options, and resource limits
src/conversion/error.rs       Typed conversion errors
src/conversion/renderer.rs    In-memory image conversion
src/bin/converter.rs         Local file CLI adapter
~~~
