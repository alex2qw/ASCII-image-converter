# Copilot instructions for ASCII Studio

## Project overview

ASCII Studio is a two-part image-to-ASCII application:

- `backend/` is a Rust 2021 crate. The reusable conversion library is in
  `backend/src/conversion/`; the CLI adapter is
  `backend/src/bin/converter.rs`; and the Axum HTTP/static-file adapter is
  `backend/src/bin/server.rs`.
- `frontend/` is a strict TypeScript React application built with Vite.
  `frontend/src/app/App.tsx` owns the editor state and UI, while
  `frontend/src/api/convert.ts` is the browser-to-server request boundary.
- The backend library decodes the uploaded image, applies the luminance and
  rendering options, draws characters with the supplied TrueType font, and
  returns PNG bytes. The HTTP service loads a default font once at startup,
  accepts optional custom font bytes, and serves `frontend/dist` as its
  fallback.

Keep the library independent of operating-system font paths: callers supply
font bytes. Keep web policy limits in the HTTP adapter/environment and pass
them into `AsciiOptions`; the library still validates limits before decoding
or rendering.

## Build, test, and lint commands

Run Rust commands from the repository root with the manifest path:

```sh
cargo fmt --manifest-path backend/Cargo.toml -- --check
cargo check --manifest-path backend/Cargo.toml
cargo test --manifest-path backend/Cargo.toml
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
```

To run one Rust test by name:

```sh
cargo test --manifest-path backend/Cargo.toml <test_name>
```

There are currently no Rust test modules and no frontend test runner or lint
script configured, so `cargo test` and a frontend type-check/build are the
available automated checks.

For the frontend:

```sh
cd frontend
npm ci
npm run typecheck
npm run build
```

`npm run dev` starts Vite and proxies `/convert` to
`http://127.0.0.1:3000`. From the repository root, `./run.sh` starts both the
Rust API and Vite; it requires Cargo, npm, and a discoverable TrueType font.
The production image can be built with `docker build -t ascii-studio .`.

When browser-level validation is useful, the project-local `.vscode/mcp.json`
provides a Playwright MCP server. Start the app with `./run.sh` first, then use
the server against the Vite URL shown by the launcher.

## Cross-layer contracts

- `AsciiOptions` is serialized as JSON using snake_case names. Keep the
  TypeScript `ConvertOptions` shape in `frontend/src/api/convert.ts` aligned
  with the Rust struct and its serde defaults.
- `POST /convert` expects multipart fields named `image`, `options`, and
  optionally `font`. Successful responses are PNG bytes with
  `Content-Type: image/png`; failures are JSON shaped as
  `{ "error": { "code": "...", "message": "..." } }`.
- The server clamps request options' `max_input_bytes` and
  `max_input_pixels` to its configured policy before calling the library.
  Preserve this boundary when adding options or resource limits.
- Keep conversion failures represented by `ConversionError` with a stable
  `code()` and human-readable display message. Update the HTTP status mapping
  in `server.rs` when adding a new error variant.
- `VITE_API_URL` selects the frontend endpoint; if absent, the browser uses
  same-origin `/convert`. `ASCII_ALLOWED_ORIGIN`, `ASCII_BIND`/`PORT`,
  `ASCII_FONT_PATH`, and the `ASCII_MAX_*` variables configure the service.
  Render and container defaults are defined in `render.yaml` and
  `Dockerfile`.

## Backend conventions

- Public library exports are re-exported from `backend/src/lib.rs`; keep
  reusable conversion behavior in the library rather than duplicating it in
  either binary.
- Validate options before doing expensive work, reject input byte limits
  before decoding, and reject decoded pixel limits immediately after decode.
- Rendering uses a character ramp from dark to light, computes output rows
  with a 0.5 terminal aspect-ratio correction, and writes an RGBA PNG in
  memory. Preserve these semantics when changing renderer behavior.
- Use `AsciiMode`'s `black_and_white` and `color` serde values. CLI mode
  aliases are intentionally limited to `black_and_white`, `bw`, and `color`.
- The server loads and validates its configured font during startup, not per
  request. Custom multipart font data is used only for that request.

## Frontend conventions

- `App.tsx` is the current feature composition point: selecting an image or
  changing any option starts a new conversion, aborting the previous request.
  Preserve abort handling and revoke object URLs when replacing preview/source
  blobs.
- Built-in and user presets use the same `ConvertOptions` object. Custom
  presets are stored under the `ascii-studio-presets` localStorage key.
- The browser rejects non-image files and applies the 10 MiB client-side
  upload check before making a request. Keep this consistent with the
  server's configured input limit.
- Keep UI control ranges and defaults consistent with Rust validation. In
  particular, the ramp is ordered dark-to-light and the background is a
  six-digit hexadecimal color.
- Use the existing Vite proxy for local API calls; do not hard-code a
  deployment URL into client code.
