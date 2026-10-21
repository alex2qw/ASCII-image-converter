---
title: ASCII Studio
emoji: 🖼️
colorFrom: gray
colorTo: indigo
sdk: docker
app_port: 7860
---

# ASCII Studio

Convert images into customizable ASCII art in the browser.

**Live demo:** add your Render URL here after deployment

ASCII Studio combines a React/Vite editor with a Rust renderer and Axum API.
It supports black-and-white and color output, custom character ramps, presets,
contrast and brightness controls, custom fonts, and PNG downloads.

## Deploy on Render

The repository includes a [`render.yaml`](./render.yaml) blueprint and a
[`Dockerfile`](./Dockerfile) for deploying the frontend and API as two free
Render services. The API uses an openly licensed DejaVu Sans Mono font from the
`fonts-dejavu` Debian package, so no macOS font needs to be uploaded.

1. Push this repository to GitHub.
2. In the [Render dashboard](https://dashboard.render.com/), choose **New >
  Blueprint** and select this repository.
3. Review the two services from `render.yaml` and deploy them on the Free plan.
4. If you change either generated service name, update both
  `VITE_API_URL` and `ASCII_ALLOWED_ORIGIN` to match the final URLs.
5. Open the frontend URL and test an image conversion.

Render free web services sleep after inactivity, so the first request after a
quiet period can take about a minute. Uploaded images are processed in memory
and are not stored by this application.

The frontend can also be deployed separately as a static site. Set
`VITE_API_URL` to the API's full `/convert` URL at build time. `PORT` is
detected automatically on Render. For another host, set
`ASCII_BIND=0.0.0.0:<port>` and set `ASCII_ALLOWED_ORIGIN` to the frontend's
origin.

## Deploy for free on Hugging Face Spaces

If Render asks for payment information, use a Hugging Face Docker Space
instead. It can host the frontend and API together without a separate
frontend service. Create a new public Space with the **Docker** SDK, then
connect or push this repository to the Space and wait for it to build.

The container listens on port `7860`, which is configured in the Dockerfile.
The resulting Space URL is the public website; no `VITE_API_URL` setting is
needed because the browser and `/convert` API share the same origin.

## Local development

Rust library and CLI usage are documented below. To run the full application
locally, use `./run.sh` after installing Rust, Node.js, and npm.

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

The function returns PNG bytes suitable for an HTTP response. It checks input bytes, image dimensions, and planned output dimensions before decoding the full pixel buffer. Configure those limits through AsciiOptions for the web application policy.

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
`ASCII_MAX_OUTPUT_PIXELS` caps rendered output at 16,000,000 pixels by default
(about 61 MiB for the RGBA buffer alone). The server clamps client-provided
`max_output_pixels` to this policy, just as it does input limits. Oversized
output returns HTTP 413 with code `output_too_large`.

CPU-heavy decoding, rendering, and PNG encoding run on Tokio's blocking thread
pool so they do not occupy async runtime workers. Set
`ASCII_MAX_CONCURRENT_CONVERSIONS` to a positive integer to control capacity
(default: `2`). A slot is reserved before buffering multipart fields and held
until the worker finishes. When all slots are occupied, new conversion requests
return HTTP 503 with `error.code: "server_busy"`; requests are not queued.
Disconnecting a client does not stop blocking work already started, so that work
retains its slot until completion. Slow uploads also occupy slots; request/upload
timeouts and deployment-wide memory limits are separate operational concerns.

The decoder also has a 128 MiB allocation budget. This is a best-effort limit
in the image library; some formats do not enforce every internal allocation.
These limits apply to each conversion, not total service memory: intermediate
buffers, PNG encoding, and concurrent requests require additional memory.
Font size must be finite and between 1 and 256 pixels. Background colours must
be exactly `#` followed by six ASCII hexadecimal digits.

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
Preview requests wait for a 300 ms pause in editing. Status labels describe the
latest conversion attempt rather than claiming a persistent server connection.
When the image, font, or settings change, the previous preview is hidden and
exports remain disabled until the new conversion succeeds. Failed conversions
can be retried. Cancelling an old browser request prevents stale results from
appearing; it does not stop server work already running.

Images larger than the configured 10 MiB default upload limit are rejected in the browser before a request is sent. Configure the server's `ASCII_MAX_INPUT_BYTES` and `ASCII_MAX_REQUEST_BYTES` together when a larger upload limit is required.

~~~sh
cd frontend
npm install
npm run dev
npm run build
~~~

## Frontend checks

Run `npm test` from `frontend/` for conversion lifecycle, status display, stale
response, export, retry, and API response tests. Run `npm run build` to type-check
and build the client.

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
