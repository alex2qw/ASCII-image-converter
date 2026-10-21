//! Real HTTP tests. Run with a licensed TrueType font:
//! ASCII_TEST_FONT=/path/to/font.ttf cargo test --test api_e2e -- --ignored
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Cursor, Read, Write},
    net::{SocketAddr, TcpStream},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

const BACKGROUND: [u8; 4] = [16, 32, 48, 255];

struct Server {
    child: Child,
    address: SocketAddr,
}

impl Server {
    fn start() -> Self {
        let font = std::env::var("ASCII_TEST_FONT")
            .expect("set ASCII_TEST_FONT to a licensed TrueType font before running API tests");
        let child = Command::new(env!("CARGO_BIN_EXE_server"))
            .env("ASCII_FONT_PATH", font)
            .env("ASCII_BIND", "127.0.0.1:0")
            .env("ASCII_MAX_REQUEST_BYTES", "1048576")
            .env("ASCII_MAX_INPUT_BYTES", "1024")
            .env("ASCII_MAX_INPUT_PIXELS", "1024")
            .env("ASCII_MAX_OUTPUT_PIXELS", "50000")
            .env("ASCII_MAX_CONCURRENT_CONVERSIONS", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start the real server binary");
        // Own the child before any assertion so startup failures also clean up.
        let mut server = Self {
            child,
            address: "127.0.0.1:0".parse().unwrap(),
        };
        let stdout = server.child.stdout.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            let line = BufReader::new(stdout).lines().next();
            let _ = sender.send(line);
        });
        let line = receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("server startup timed out")
            .expect("server exited without reporting its listening address")
            .expect("read the server startup message");
        reader.join().unwrap();
        server.address = line
            .strip_prefix("ASCII image converter listening on ")
            .expect("unexpected server startup message")
            .parse()
            .expect("parse the actual OS-assigned port");
        server
    }

    fn convert(&self, image: Option<&[u8]>, options: &Value, font: Option<&[u8]>) -> HttpResponse {
        let boundary = "ascii-studio-e2e-boundary";
        let mut body = Vec::new();
        if let Some(image) = image {
            write!(body, "--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"source.png\"\r\nContent-Type: image/png\r\n\r\n").unwrap();
            body.extend_from_slice(image);
            body.extend_from_slice(b"\r\n");
        }
        write!(
            body,
            "--{boundary}\r\nContent-Disposition: form-data; name=\"options\"\r\n\r\n{options}\r\n"
        )
        .unwrap();
        if let Some(font) = font {
            write!(body, "--{boundary}\r\nContent-Disposition: form-data; name=\"font\"; filename=\"custom.ttf\"\r\nContent-Type: font/ttf\r\n\r\n").unwrap();
            body.extend_from_slice(font);
            body.extend_from_slice(b"\r\n");
        }
        write!(body, "--{boundary}--\r\n").unwrap();

        let mut stream = TcpStream::connect_timeout(&self.address, Duration::from_secs(5))
            .expect("connect to the real HTTP listener");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        write!(stream, "POST /convert HTTP/1.1\r\nHost: {}\r\nContent-Type: multipart/form-data; boundary={boundary}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", self.address, body.len()).unwrap();
        stream.write_all(&body).unwrap();
        let mut response = Vec::new();
        // Test responses are small; cap reads so a regression cannot consume
        // unlimited memory in the test client itself.
        stream
            .take(2 * 1024 * 1024)
            .read_to_end(&mut response)
            .unwrap();
        HttpResponse::parse(response)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct HttpResponse {
    status: u16,
    content_type: String,
    body: Vec<u8>,
}

impl HttpResponse {
    fn parse(bytes: Vec<u8>) -> Self {
        let split = bytes
            .windows(4)
            .position(|part| part == b"\r\n\r\n")
            .expect("HTTP response must have complete headers");
        let headers = std::str::from_utf8(&bytes[..split]).unwrap();
        let mut lines = headers.split("\r\n");
        let status = lines
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let headers: Vec<_> = lines.map(|line| line.split_once(':').unwrap()).collect();
        let header = |name: &str| {
            headers
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.trim())
                .unwrap_or("")
        };
        // Axum's buffered PNG/JSON responses have known lengths. Verify the
        // entire body arrived rather than accepting a truncated response.
        let content_length: usize = header("content-length")
            .parse()
            .expect("buffered response length");
        let body = bytes[split + 4..].to_vec();
        assert_eq!(body.len(), content_length, "truncated HTTP response");
        Self {
            status,
            content_type: header("content-type").to_owned(),
            body,
        }
    }

    fn expect_error(&self, status: u16, code: &str) {
        assert_eq!(
            self.status,
            status,
            "{}",
            String::from_utf8_lossy(&self.body)
        );
        assert_eq!(self.content_type, "application/json");
        let body: Value = serde_json::from_slice(&self.body).expect("valid JSON error body");
        assert_eq!(body["error"]["code"], code);
        assert!(!body["error"]["message"].as_str().unwrap().is_empty());
    }

    fn expect_png(&self) -> RgbaImage {
        assert_eq!(self.status, 200, "{}", String::from_utf8_lossy(&self.body));
        assert_eq!(self.content_type, "image/png");
        assert_eq!(image::guess_format(&self.body).unwrap(), ImageFormat::Png);
        let image = image::load_from_memory(&self.body)
            .expect("decode the complete returned PNG")
            .to_rgba8();
        // 8 columns x 4 rows, with 10x18 pixel character cells.
        assert_eq!(image.dimensions(), (80, 72));
        assert!(image.pixels().any(|pixel| pixel.0 == BACKGROUND));
        assert!(
            image.pixels().any(|pixel| pixel.0 != BACKGROUND),
            "output must contain rendered glyphs, not just a blank canvas"
        );
        image
    }
}

fn source_png(width: u32, height: u32) -> Vec<u8> {
    let image = RgbaImage::from_pixel(width, height, Rgba([220, 40, 20, 255]));
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn options() -> Value {
    // A repeated glyph keeps the image nonblank regardless of luminance,
    // without comparing platform-dependent font rasterization snapshots.
    json!({ "max_columns": 8, "ramp": "##", "background": "#102030" })
}

#[test]
#[ignore = "requires ASCII_TEST_FONT and permission to bind localhost"]
fn black_and_white_upload_returns_a_decodable_nonblank_png() {
    let response = Server::start().convert(Some(&source_png(32, 32)), &options(), None);
    let image = response.expect_png();
    assert!(
        image
            .pixels()
            .any(|p| p[0] > 200 && p[1] > 200 && p[2] > 200),
        "black-and-white glyphs should contain bright ink"
    );
}

#[test]
#[ignore = "requires ASCII_TEST_FONT and permission to bind localhost"]
fn color_mode_and_custom_font_work_through_multipart_upload() {
    let mut options = options();
    options["mode"] = json!("color");
    let font = std::fs::read(std::env::var("ASCII_TEST_FONT").unwrap()).unwrap();
    let response = Server::start().convert(Some(&source_png(32, 32)), &options, Some(&font));
    let image = response.expect_png();
    assert!(
        image
            .pixels()
            .any(|p| p[0] > 100 && p[0] > p[1] && p[1] > p[2]),
        "color glyphs should preserve the red source color"
    );
}

#[test]
#[ignore = "requires ASCII_TEST_FONT and permission to bind localhost"]
fn invalid_upload_returns_json_and_does_not_break_subsequent_conversion() {
    let server = Server::start();
    server
        .convert(Some(b"not an image"), &options(), None)
        .expect_error(400, "invalid_image");
    server
        .convert(Some(&source_png(32, 32)), &options(), None)
        .expect_png();
}

#[test]
#[ignore = "requires ASCII_TEST_FONT and permission to bind localhost"]
fn malformed_unicode_color_returns_json_without_crashing_the_server() {
    let server = Server::start();
    let mut invalid = options();
    invalid["background"] = json!("#a€bc");
    server
        .convert(Some(&source_png(32, 32)), &invalid, None)
        .expect_error(400, "invalid_options");
    server
        .convert(Some(&source_png(32, 32)), &options(), None)
        .expect_png();
}

#[test]
#[ignore = "requires ASCII_TEST_FONT and permission to bind localhost"]
fn client_cannot_raise_the_server_input_byte_limit() {
    let mut options = options();
    options["max_input_bytes"] = json!(10_000_000);
    Server::start()
        .convert(Some(&vec![0; 2048]), &options, None)
        .expect_error(413, "input_too_large");
}

#[test]
#[ignore = "requires ASCII_TEST_FONT and permission to bind localhost"]
fn client_cannot_raise_the_server_input_pixel_limit() {
    let mut options = options();
    options["max_input_pixels"] = json!(10_000_000);
    Server::start()
        .convert(Some(&source_png(64, 32)), &options, None)
        .expect_error(413, "image_too_large");
}

#[test]
#[ignore = "requires ASCII_TEST_FONT and permission to bind localhost"]
fn client_cannot_raise_the_server_output_limit() {
    let mut options = options();
    options["max_columns"] = json!(32);
    options["character_width"] = json!(64);
    options["character_height"] = json!(64);
    options["max_output_pixels"] = json!(10_000_000);
    Server::start()
        .convert(Some(&source_png(32, 32)), &options, None)
        .expect_error(413, "output_too_large");
}

#[test]
#[ignore = "requires ASCII_TEST_FONT and permission to bind localhost"]
fn oversized_multipart_body_returns_request_limit_error() {
    Server::start()
        .convert(Some(&vec![0; 1048576]), &options(), None)
        .expect_error(413, "request_too_large");
}

#[test]
#[ignore = "requires ASCII_TEST_FONT and permission to bind localhost"]
fn missing_image_returns_a_structured_validation_error() {
    Server::start()
        .convert(None, &options(), None)
        .expect_error(400, "invalid_options");
}
