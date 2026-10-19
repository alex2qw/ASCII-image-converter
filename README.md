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
ASCII_FONT_PATH=/path/to/monospace.ttf cargo run --manifest-path backend/Cargo.toml -- input_image.png output_image.png
ASCII_FONT_PATH=/path/to/monospace.ttf ASCII_MODE=color cargo run --manifest-path backend/Cargo.toml -- input_image.png output_image.png
~~~

Supported modes are black_and_white (the default), bw, and color.

## HTTP service

Run the HTTP adapter with a licensed TrueType font loaded once at startup:

~~~sh
ASCII_FONT_PATH=/path/to/monospace.ttf cargo run --manifest-path backend/Cargo.toml --bin server
~~~

`POST /convert` accepts a `multipart/form-data` request with an `image` file
field and an `options` field containing serialized `AsciiOptions` JSON. The
response is PNG bytes (`Content-Type: image/png`). For example:

~~~sh
curl -X POST http://127.0.0.1:3000/convert \
  -F image=@input_image.png \
  -F 'options={"mode":"color","max_columns":80}'
~~~

The service rejects oversized request bodies before parsing them. Its
`ASCII_MAX_REQUEST_BYTES`, `ASCII_MAX_INPUT_BYTES`, and
`ASCII_MAX_INPUT_PIXELS` environment variables provide request and conversion
limits (with safe defaults); `ASCII_BIND` controls the listen address.
Conversion failures return JSON with a stable `error.code` and human-readable
`error.message`. Invalid input/options use 400, size limits use 413, and
startup/font failures are reported as server errors.

## Project layout

~~~text
backend/src/lib.rs                    Public library API
backend/src/conversion/options.rs     Modes, rendering options, and resource limits
backend/src/conversion/error.rs       Typed conversion errors
backend/src/conversion/renderer.rs    In-memory image conversion
backend/src/bin/converter.rs          Local file CLI adapter
backend/src/bin/server.rs             Axum HTTP service adapter
~~~

## Web frontend

The Vite client lives in `frontend/` and runs with `npm run dev` from that directory. Development proxies `/convert` to the local Axum adapter at `http://127.0.0.1:3000`. For a deployed frontend, set `VITE_API_URL` to the API URL or use a same-origin reverse proxy. The request is `multipart/form-data` with `image`, optional `font`, and JSON `options` parts; the response is a PNG (`image/png`) or `{ "error": { "message": "..." } }` on failure. The Rust library maps all editor fields (`mode`, `max_columns`, `font_size`, `character_width`, `character_height`, `ramp`, `contrast`, `brightness`, `invert`, and `background`) to `AsciiOptions`. The browser never uploads an image until one is selected, and custom presets are stored only in localStorage.
Images larger than the configured 10 MiB default upload limit are rejected in the browser before a request is sent. Configure the server's `ASCII_MAX_INPUT_BYTES` and `ASCII_MAX_REQUEST_BYTES` together when a larger upload limit is required.

~~~sh
cd frontend
npm install
npm run dev
npm run build
~~~

## Run everything with one command

On macOS, make the launcher executable once:

~~~sh
chmod +x run.sh
~~~

Then start both the Rust API and the Vite frontend with:

~~~sh
./run.sh
~~~

The launcher finds a system TrueType font automatically. To use a specific
font, set `ASCII_FONT_PATH` before running it:

~~~sh
ASCII_FONT_PATH=/path/to/monospace.ttf ./run.sh
~~~

Press `Ctrl+C` to stop both services.
