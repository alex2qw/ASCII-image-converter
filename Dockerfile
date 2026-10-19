FROM rust:1-bookworm AS builder

WORKDIR /app
COPY backend/Cargo.toml backend/Cargo.lock backend/
COPY backend/src backend/src
RUN cargo build --release --manifest-path backend/Cargo.toml --bin server

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install --no-install-recommends -y fonts-dejavu \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/backend/target/release/server /usr/local/bin/ascii-image-server

ENV ASCII_FONT_PATH=/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf
ENV ASCII_MAX_INPUT_BYTES=10485760
ENV ASCII_MAX_REQUEST_BYTES=10551296
ENV ASCII_MAX_INPUT_PIXELS=16000000

CMD ["/usr/local/bin/ascii-image-server"]
