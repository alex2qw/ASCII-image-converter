use ab_glyph::FontArc;
use ascii_image_converter::{convert_image_to_png, AsciiOptions, ConversionError};
use axum::{
    extract::{DefaultBodyLimit, Multipart, State},
    http::{header, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::Serialize;
use std::{env, net::SocketAddr, sync::Arc};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tower_http::{cors::CorsLayer, services::ServeDir};

#[derive(Clone)]
struct AppState {
    font: Arc<Vec<u8>>,
    conversion_slots: Arc<Semaphore>,
    max_input_bytes: usize,
    max_input_pixels: u64,
    max_output_pixels: u64,
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

#[derive(Debug)]
enum ServiceError {
    Conversion(ConversionError),
    Busy,
    WorkerFailed,
}

impl From<ConversionError> for ServiceError {
    fn from(error: ConversionError) -> Self {
        Self::Conversion(error)
    }
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Busy => (
                StatusCode::SERVICE_UNAVAILABLE,
                "server_busy",
                "All conversion slots are in use. Please try again shortly.".to_owned(),
            ),
            Self::WorkerFailed => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "conversion_worker_failed",
                "The conversion worker failed. Please try again.".to_owned(),
            ),
            Self::Conversion(error) => {
                let status = match &error {
                    ConversionError::InputTooLarge { .. }
                    | ConversionError::ImageTooLarge { .. }
                    | ConversionError::OutputTooLarge { .. }
                    | ConversionError::RequestTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
                    ConversionError::Image(_) | ConversionError::InvalidOptions(_) => {
                        StatusCode::BAD_REQUEST
                    }
                    ConversionError::MissingFont | ConversionError::InvalidFont => {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                };
                (status, error.code(), error.to_string())
            }
        };
        (
            status,
            Json(ErrorResponse {
                error: ErrorDetails { code, message },
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
    max_output_pixels: u64,
    allowed_origin: HeaderValue,
    max_concurrent_conversions: usize,
}

impl ServerConfig {
    fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let default_bind = env::var("PORT")
            .map(|port| format!("0.0.0.0:{port}"))
            .unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
        let bind = env::var("ASCII_BIND").unwrap_or(default_bind).parse()?;
        let font_path = env::var("ASCII_FONT_PATH")
            .map_err(|_| "ASCII_FONT_PATH must point to a licensed .ttf font file")?;
        let max_input_bytes = env_usize("ASCII_MAX_INPUT_BYTES", 10 * 1024 * 1024)?;
        let max_request_bytes = env_usize(
            "ASCII_MAX_REQUEST_BYTES",
            max_input_bytes.saturating_add(64 * 1024),
        )?;
        let max_input_pixels = env_u64("ASCII_MAX_INPUT_PIXELS", 16_000_000)?;
        let max_output_pixels = env_u64("ASCII_MAX_OUTPUT_PIXELS", 16_000_000)?;
        let max_concurrent_conversions = env_usize("ASCII_MAX_CONCURRENT_CONVERSIONS", 2)?;
        if max_concurrent_conversions == 0 || max_concurrent_conversions > Semaphore::MAX_PERMITS {
            return Err("ASCII_MAX_CONCURRENT_CONVERSIONS must be positive and no greater than Semaphore::MAX_PERMITS".into());
        }
        let allowed_origin = env::var("ASCII_ALLOWED_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:5173".to_owned())
            .parse()?;
        if max_request_bytes == 0
            || max_input_bytes == 0
            || max_input_pixels == 0
            || max_output_pixels == 0
        {
            return Err("request, input, and output limits must be greater than zero".into());
        }
        Ok(Self {
            bind,
            font_path,
            max_request_bytes,
            max_input_bytes,
            max_input_pixels,
            max_output_pixels,
            allowed_origin,
            max_concurrent_conversions,
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
        conversion_slots: Arc::new(Semaphore::new(config.max_concurrent_conversions)),
        max_input_bytes: config.max_input_bytes,
        max_input_pixels: config.max_input_pixels,
        max_output_pixels: config.max_output_pixels,
    };
    let cors = CorsLayer::new()
        .allow_origin(config.allowed_origin)
        .allow_methods([Method::POST])
        .allow_headers([header::CONTENT_TYPE, header::ACCEPT]);
    let app = Router::new()
        .route("/convert", post(convert))
        .with_state(state)
        .layer(DefaultBodyLimit::max(config.max_request_bytes))
        .layer(cors)
        .fallback_service(ServeDir::new(
            env::var("ASCII_FRONTEND_DIR").unwrap_or_else(|_| "frontend/dist".to_owned()),
        ));
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
    // Reserve capacity before buffering multipart fields. Fail fast rather than
    // retaining uploaded images in an unbounded queue of waiting requests.
    let permit = reserve_conversion(&state.conversion_slots)?;
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
    options.max_output_pixels = options.max_output_pixels.min(state.max_output_pixels);
    let default_font = state.font;
    let output = run_conversion(permit, move || {
        let font = custom_font.as_deref().unwrap_or(default_font.as_slice());
        convert_image_to_png(&image, font, &options)
    })
    .await?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/png")], output))
}

fn reserve_conversion(slots: &Arc<Semaphore>) -> Result<OwnedSemaphorePermit, ServiceError> {
    slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ServiceError::Busy)
}

async fn run_conversion<F>(permit: OwnedSemaphorePermit, work: F) -> Result<Vec<u8>, ServiceError>
where
    F: FnOnce() -> Result<Vec<u8>, ConversionError> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        // The worker owns the permit: disconnecting/cancelling the handler must
        // not release capacity while its blocking work is still running.
        let _permit = permit;
        work()
    })
    .await
    .map_err(|error| {
        eprintln!("conversion worker failed: {error}");
        ServiceError::WorkerFailed
    })?
    .map_err(ServiceError::from)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_output_is_a_payload_too_large_response() {
        let error = ConversionError::OutputTooLarge {
            actual: 17_000_000,
            max: 16_000_000,
        };
        assert_eq!(error.code(), "output_too_large");
        assert_eq!(
            ServiceError::from(error).into_response().status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn worker_does_not_block_runtime_and_keeps_slot_after_handler_cancellation() {
        let slots = Arc::new(Semaphore::new(1));
        let permit = reserve_conversion(&slots).unwrap();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let handler = tokio::spawn(run_conversion(permit, move || {
            started_tx.send(()).unwrap();
            // A deadline makes a regression fail instead of hanging the test.
            release_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            Ok(vec![1, 2, 3])
        }));
        // With one runtime thread, this can resume while the worker waits only
        // because the conversion runs on a separate blocking thread.
        started_rx.await.unwrap();
        assert!(matches!(
            reserve_conversion(&slots),
            Err(ServiceError::Busy)
        ));
        handler.abort();
        assert!(handler.await.unwrap_err().is_cancelled());
        assert!(matches!(
            reserve_conversion(&slots),
            Err(ServiceError::Busy)
        ));
        release_tx.send(()).unwrap();
        // The slot becomes available only when the detached worker exits.
        let _permit = slots.acquire_owned().await.unwrap();
    }

    #[tokio::test]
    async fn successful_and_failed_workers_release_capacity() {
        let slots = Arc::new(Semaphore::new(1));
        let output = run_conversion(reserve_conversion(&slots).unwrap(), || Ok(vec![42]))
            .await
            .unwrap();
        assert_eq!(output, vec![42]);
        assert_eq!(slots.available_permits(), 1);
        let result = run_conversion(reserve_conversion(&slots).unwrap(), || {
            Err(ConversionError::InvalidFont)
        })
        .await;
        assert!(matches!(
            result,
            Err(ServiceError::Conversion(ConversionError::InvalidFont))
        ));
        assert_eq!(slots.available_permits(), 1);
    }

    #[tokio::test]
    async fn worker_panic_returns_server_error_and_releases_capacity() {
        let slots = Arc::new(Semaphore::new(1));
        let error = run_conversion(reserve_conversion(&slots).unwrap(), || panic!("test panic"))
            .await
            .unwrap_err();
        assert_eq!(
            error.into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(slots.available_permits(), 1);
    }

    #[test]
    fn full_capacity_returns_service_unavailable() {
        let slots = Arc::new(Semaphore::new(1));
        let permit = reserve_conversion(&slots).unwrap();
        let error = reserve_conversion(&slots).unwrap_err();
        assert_eq!(
            error.into_response().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        drop(permit);
        assert!(reserve_conversion(&slots).is_ok());
    }
}
