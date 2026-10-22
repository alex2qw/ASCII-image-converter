# Development and API reference

[← Project overview](../README.md)

Run the commands below from the repository root unless a section says otherwise.
Use stable Rust/Cargo, Node.js 24 with npm, and a licensed TrueType font.

## Run the services separately

Install frontend dependencies:

```sh
npm --prefix frontend ci
```

Start the API in one terminal:

```sh
ASCII_FONT_PATH=/absolute/path/to/monospace.ttf \
  cargo run --locked --manifest-path backend/Cargo.toml --bin server
```

Start Vite in another:

```sh
npm --prefix frontend run dev
```

Vite proxies `/convert` to `http://127.0.0.1:3000`. The browser sends requests to its own Vite origin during development. For the combined launcher, use `./run.sh`; if no system font is found, supply `ASCII_FONT_PATH` explicitly.

## Library API

The public library accepts image bytes, font bytes, and validated rendering options, and returns PNG bytes:

```rust
use ascii_image_converter::{convert_image_to_png, AsciiMode, AsciiOptions, ConversionError};

fn convert(image: &[u8], font: &[u8]) -> Result<Vec<u8>, ConversionError> {
    let options = AsciiOptions {
        mode: AsciiMode::Color,
        ..AsciiOptions::default()
    };
    convert_image_to_png(image, font, &options)
}
```

The library knows nothing about file paths, HTTP, or server configuration. The HTTP adapter loads default font bytes at startup and passes them, or a request’s custom font bytes, into the library.

## CLI

The package contains two binaries, so select the converter explicitly:

```sh
ASCII_FONT_PATH=/absolute/path/to/monospace.ttf \
  cargo run --locked --manifest-path backend/Cargo.toml \
  --bin ascii_image_converter -- input.png output.png

ASCII_FONT_PATH=/absolute/path/to/monospace.ttf ASCII_MODE=color \
  cargo run --locked --manifest-path backend/Cargo.toml \
  --bin ascii_image_converter -- input.png output-color.png
```

`ASCII_MODE` accepts `black_and_white` (default), `bw`, and `color`. The CLI exposes mode selection and uses the other `AsciiOptions` defaults; use the library or HTTP API for the full option set.

## HTTP API

`POST /convert` accepts `multipart/form-data`:

| Field | Required | Content |
| :--- | :--- | :--- |
| `image` | Yes | Image file bytes |
| `options` | No | JSON object; omitted options use library defaults |
| `font` | No | Custom font bytes for this conversion |

Successful responses contain PNG bytes with `Content-Type: image/png`.
Save the response to a file rather than printing binary data in the terminal:

```sh
curl --fail-with-body http://127.0.0.1:3000/convert \
  -F 'image=@input.png' \
  -F 'options={"mode":"color","max_columns":120,"invert":true}' \
  --output output.png
```

On an error, the saved response body is JSON rather than a PNG. Error responses use this shape:

```json
{
  "error": {
    "code": "output_too_large",
    "message": "rendered image is too large: 17000000 pixels exceeds the 16000000 pixel limit"
  }
}
```

| HTTP status | Cases |
| :--- | :--- |
| `400` | Invalid image data, malformed options, missing image, or malformed multipart data |
| `413` | Request-body, input-byte, input-pixel, or output-pixel limit exceeded |
| `503` | All conversion admission slots are occupied (`server_busy`) |
| `500` | Missing/invalid font data or a failed blocking worker |

An invalid configured startup font prevents the server from starting. Invalid custom font uploads currently use the same font error mapping and return `500`.

## Options and limits

Rendering options include `mode`, `max_columns`, `font_size`, `character_width`, `character_height`, `ramp`, `contrast`, `brightness`, `invert`, and `background`. See [the option definitions and validation](../backend/src/conversion/options.rs) for defaults and accepted ranges. A font size must be finite and between 1 and 256; a background must be `#` followed by six ASCII hexadecimal digits.

The limit fields `max_input_bytes`, `max_input_pixels`, and `max_output_pixels` can tighten library limits. HTTP clients cannot raise them above server policy: the adapter takes the minimum of the supplied and server-configured values.

| Environment variable | Default | Purpose |
| :--- | :--- | :--- |
| `ASCII_FONT_PATH` | Required | Licensed default font loaded at startup |
| `ASCII_BIND` | `127.0.0.1:3000`, or `0.0.0.0:$PORT` when `PORT` is set | Listen address; an explicit value takes precedence |
| `ASCII_ALLOWED_ORIGIN` | `http://localhost:5173` | Allowed browser origin for direct API requests |
| `ASCII_MAX_REQUEST_BYTES` | Input-byte limit + 64 KiB | Total multipart body, including font and options |
| `ASCII_MAX_INPUT_BYTES` | 10 MiB | Uploaded image size |
| `ASCII_MAX_INPUT_PIXELS` | 16,000,000 | Advertised input pixel count checked before full decoding |
| `ASCII_MAX_OUTPUT_PIXELS` | 16,000,000 | Planned rendered pixel count checked before allocation |
| `ASCII_MAX_CONCURRENT_CONVERSIONS` | `2` | Admission slots held during upload parsing and conversion |
| `ASCII_FRONTEND_DIR` | `frontend/dist` | Static files served by the API process |
| `VITE_API_URL` | `/convert` | Frontend endpoint, set at frontend build time |

The browser has a fixed 10 MiB image-upload check in [the API client](../frontend/src/api/convert.ts). Increasing server limits alone does not increase this browser limit. Account for custom font bytes and multipart overhead when changing the request-body budget.

The decoder has an additional 128 MiB allocation budget. Some formats enforce this only on a best-effort basis. An output budget of 16 million pixels permits about 61 MiB for its RGBA buffer alone, before intermediate buffers and PNG encoding. These are per-conversion limits, not a guarantee of total process memory.

## Request and preview lifecycle

The API reserves a slot before buffering multipart fields. If none is available, it returns `503` immediately. Once admitted, CPU-heavy decoding, rendering, and encoding run through `spawn_blocking`. The worker owns the slot until it exits, including when its handler is cancelled; a disconnected client does not stop blocking work already running. Slow uploads also occupy slots, so upload timeouts and process-level resource controls require separate consideration.

The editor waits 300 ms after changes before submitting, aborts obsolete browser requests, and refuses to publish stale responses. Changing the selected image, font, or options hides the previous preview and disables exports until a matching result succeeds. Failed requests can be retried. Presets are saved in localStorage; image uploads are processed in memory without application-level file persistence. The application does not implement account storage or a server-side image gallery.

## Automated checks

```sh
cargo fmt --manifest-path backend/Cargo.toml --all -- --check
cargo test --locked --manifest-path backend/Cargo.toml
cargo clippy --locked --manifest-path backend/Cargo.toml --all-targets -- -D warnings
npm --prefix frontend ci
npm --prefix frontend test
npm --prefix frontend run build
```

The build script runs TypeScript checks before Vite produces the frontend bundle. `npm --prefix frontend run typecheck` runs the type check separately.

### Real HTTP tests

```sh
ASCII_TEST_FONT=/absolute/path/to/monospace.ttf \
  cargo test --locked --manifest-path backend/Cargo.toml --test api_e2e -- --ignored
```

On macOS, a suitable local path is `/System/Library/Fonts/Supplemental/Courier New.ttf` (quote paths containing spaces).

[The HTTP suite](../backend/tests/api_e2e.rs) launches the actual compiled server on OS-assigned localhost ports. It generates source PNGs in memory, uploads them through real TCP/HTTP multipart requests, and decodes returned PNGs to check dimensions and visible glyphs. It also checks colour rendering with a custom font, structured errors, server policy, and successful requests after failures. Each test stops its child process when it finishes or fails.

These tests are marked `#[ignore]` in the ordinary test run because they need a licensed font and socket access. The explicit command above must be run to exercise them. Missing font configuration fails that run rather than silently skipping it.

[CI](../.github/workflows/ci.yml) runs on pushes to `main`, pull requests, and manual dispatches. Separate backend/frontend jobs run the checks; the backend job installs DejaVu Sans Mono and explicitly runs the HTTP suite. CI reports failures. Making these checks mandatory before merging requires a separate repository ruleset.

Coverage includes resource validation, worker capacity/cancellation, API responses, preview freshness, retries, and export state. Frontend component tests use jsdom and mocked API responses; the HTTP suite performs real conversion. Neither replaces automated browser interaction or deployment/container testing.

## Repository layout

```text
backend/src/conversion/             Shared rendering, options, and errors
backend/src/bin/converter.rs        File-based CLI
backend/src/bin/server.rs           HTTP/static-file server and worker admission
backend/tests/api_e2e.rs            Real HTTP conversion tests
frontend/src/app/App.tsx            Editor interface
frontend/src/app/hooks/             Conversion lifecycle and tests
frontend/src/api/                   Browser API client and tests
docs/                              Development, deployment, and media notes
.github/workflows/ci.yml            Automated checks
```
