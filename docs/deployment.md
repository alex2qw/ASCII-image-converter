# Deployment

[← Project overview](../README.md) · [Configuration reference](development.md#options-and-limits)

The repository includes a Docker image definition and hosting configuration. The image serves the built frontend and API together; a separate static frontend is also supported. Hosting setup is independent of the test workflow—CI does not deploy the app.

## Docker

From the repository root:

```sh
docker build -t ascii-studio .
docker run --rm -p 7860:7860 ascii-studio
```

Open `http://localhost:7860`. The image builds the frontend and Rust server, installs DejaVu Sans Mono from Debian’s `fonts-dejavu` package, and listens on port `7860`. No macOS font file is needed or copied into the image.

The browser uses the same-origin `/convert` endpoint, so `VITE_API_URL` is unnecessary for this combined setup. Use `-e` flags to override server settings such as `ASCII_MAX_CONCURRENT_CONVERSIONS`; see the [configuration table](development.md#options-and-limits).

## Render

[`render.yaml`](../render.yaml) defines a Docker API service and a Vite static frontend. Its service names and URLs must agree.

1. Connect this repository through the Render dashboard and create a Blueprint from `render.yaml`.
2. Review the service configuration and plan settings before deploying.
3. Match frontend `VITE_API_URL` to the API’s full `/convert` URL and API `ASCII_ALLOWED_ORIGIN` to the frontend origin. Update the blueprint values if generated names or URLs differ.
4. Deploy both services, open the frontend URL, and verify a conversion and PNG download.

`VITE_API_URL` is a build-time value, so rebuild the frontend after changing it. The server detects Render’s `PORT` environment variable automatically. Service pricing and idle behaviour depend on the selected hosting plan; this repository does not guarantee free hosting or startup latency.

See Render’s [Blueprint reference](https://render.com/docs/blueprint-spec) for the hosting configuration fields.

## Hugging Face Docker Spaces

Create a Space using the **Docker** SDK and push the project files to that Space’s repository. Keep the root README YAML metadata with `sdk: docker` and `app_port: 7860`; the Dockerfile listens on the same port. The built frontend and API then share the Space origin, using `/convert` without a separate frontend service.

See the official [Docker Spaces guide](https://huggingface.co/docs/hub/spaces-sdks-docker) for Space setup and environment configuration. Choose hosting resources appropriate to the configured conversion limits.

## Separate frontend and API

Build the frontend with the deployed API endpoint:

```sh
VITE_API_URL=https://your-api.example/convert npm --prefix frontend run build
```

Publish `frontend/dist` to the static host. Configure the API with `ASCII_ALLOWED_ORIGIN=https://your-frontend.example` and a listen address appropriate to the host. The server uses `0.0.0.0:$PORT` when `PORT` is set; otherwise set `ASCII_BIND` explicitly when external access is needed.

## Before sharing a deployment

Verify an ordinary conversion, a rejected invalid upload, and a PNG download on the deployed URL. Check logs and memory use with representative images. Per-conversion limits and bounded workers do not establish a process-wide memory guarantee; worker cancellation, slow-upload timeouts, and infrastructure resource controls have separate tradeoffs described in the [development guide](development.md#request-and-preview-lifecycle).
