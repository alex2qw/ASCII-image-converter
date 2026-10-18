use ab_glyph::FontArc;
use ascii_image_converter::{convert_image_to_png, AsciiOptions, ConversionError};
use axum::{
    extract::{DefaultBodyLimit, Multipart, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::Serialize;
use std::{env, net::SocketAddr, sync::Arc};

#[derive(Clone)]
struct AppState {
    font: Arc<Vec<u8>>,
    max_input_bytes: usize,
    max_input_pixels: u64,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorDetails,
}

#[derive(Serialize)]
struct ErrorDetails {
    code: &'static str,
    message: String,
}

struct ServiceError(ConversionError);

impl From<ConversionError> for ServiceError {
    fn from(error: ConversionError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let error = self.0;
        let status = match &error {
            ConversionError::InputTooLarge { .. } | ConversionError::ImageTooLarge { .. } => {
                StatusCode::PAYLOAD_TOO_LARGE
            }
            ConversionError::RequestTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            ConversionError::Image(_) | ConversionError::InvalidOptions(_) => {
                StatusCode::BAD_REQUEST
            }
            ConversionError::MissingFont | ConversionError::InvalidFont => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        (
            status,
            Json(ErrorResponse {
                error: ErrorDetails {
                    code: error.code(),
                    message: error.to_string(),
                },
            }),
        )
            .into_response()
    }
}

#[derive(Debug)]
struct ServerConfig {
    bind: SocketAddr,
    font_path: String,
    max_request_bytes: usize,
    max_input_bytes: usize,
    max_input_pixels: u64,
}

impl ServerConfig {
    fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let bind = env::var("ASCII_BIND")
            .unwrap_or_else(|_| "127.0.0.1:3000".to_owned())
            .parse()?;
        let font_path = env::var("ASCII_FONT_PATH")
            .map_err(|_| "ASCII_FONT_PATH must point to a licensed .ttf font file")?;
        let max_input_bytes = env_usize("ASCII_MAX_INPUT_BYTES", 10 * 1024 * 1024)?;
        let max_request_bytes = env_usize(
            "ASCII_MAX_REQUEST_BYTES",
            max_input_bytes.saturating_add(64 * 1024),
        )?;
        let max_input_pixels = env_u64("ASCII_MAX_INPUT_PIXELS", 16_000_000)?;
        if max_request_bytes == 0 || max_input_bytes == 0 || max_input_pixels == 0 {
            return Err("request and input limits must be greater than zero".into());
        }
        Ok(Self {
            bind,
            font_path,
            max_request_bytes,
            max_input_bytes,
            max_input_pixels,
        })
    }
}

fn env_usize(name: &str, default: usize) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .parse()?)
}

fn env_u64(name: &str, default: u64) -> Result<u64, Box<dyn std::error::Error>> {
    Ok(env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .parse()?)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig::from_env()?;
    let font = Arc::new(std::fs::read(&config.font_path)?);
    FontArc::try_from_vec((*font).clone())
        .map_err(|_| format!("font at {} is invalid", config.font_path))?;
    let state = AppState {
        font,
        max_input_bytes: config.max_input_bytes,
        max_input_pixels: config.max_input_pixels,
    };
    let app = Router::new()
        .route("/convert", post(convert))
        .with_state(state)
        .layer(DefaultBodyLimit::max(config.max_request_bytes));
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    println!(
        "ASCII image converter listening on {}",
        listener.local_addr()?
    );
    axum::serve(listener, app).await?;
    Ok(())
}

async fn convert(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, ServiceError> {
    let mut image = None;
    let mut custom_font = None;
    let mut options = AsciiOptions::default();

    while let Some(mut field) = multipart.next_field().await.map_err(map_multipart_error)? {
        match field.name() {
            Some("image") => {
                image = Some(field.bytes().await.map_err(map_multipart_error)?);
            }
            Some("options") => {
                let bytes = field.bytes().await.map_err(map_multipart_error)?;
                options = serde_json::from_slice(&bytes)
                    .map_err(|error| ConversionError::InvalidOptions(error.to_string()))?;
            }
            Some("font") => {
                custom_font = Some(field.bytes().await.map_err(map_multipart_error)?);
            }
            _ => while field.chunk().await.map_err(map_multipart_error)?.is_some() {},
        }
    }

    let image = image.ok_or_else(|| {
        ConversionError::InvalidOptions("multipart field `image` is required".to_owned())
    })?;
    options.max_input_bytes = options.max_input_bytes.min(state.max_input_bytes);
    options.max_input_pixels = options.max_input_pixels.min(state.max_input_pixels);
    let font = custom_font.as_deref().unwrap_or(state.font.as_slice());
    let output = convert_image_to_png(&image, font, &options)?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/png")], output))
}

fn map_multipart_error(error: axum::extract::multipart::MultipartError) -> ConversionError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ConversionError::RequestTooLarge
    } else {
        ConversionError::InvalidOptions(format!(
            "multipart request could not be read: {}",
            error.body_text()
        ))
    }
}
