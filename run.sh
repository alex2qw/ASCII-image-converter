#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

if ! command -v cargo >/dev/null 2>&1; then
  echo "Error: Rust/Cargo is required. Install it from https://rustup.rs/" >&2
  exit 1
fi

if ! command -v npm >/dev/null 2>&1; then
  echo "Error: Node.js/npm is required. Install it from https://nodejs.org/" >&2
  exit 1
fi

if [[ -z "${ASCII_FONT_PATH:-}" ]]; then
  for candidate in \
    "/System/Library/Fonts/Supplemental/Arial.ttf" \
    "/System/Library/Fonts/Supplemental/Courier New.ttf" \
    "/Library/Fonts/Arial.ttf" \
    "/Library/Fonts/Courier New.ttf"
  do
    if [[ -f "$candidate" ]]; then
      ASCII_FONT_PATH="$candidate"
      break
    fi
  done
fi

if [[ -z "${ASCII_FONT_PATH:-}" || ! -f "$ASCII_FONT_PATH" ]]; then
  echo "Error: no TrueType font was found." >&2
  echo "Set ASCII_FONT_PATH to a .ttf file and run this script again." >&2
  exit 1
fi

if [[ ! -d node_modules ]]; then
  echo "Installing frontend dependencies..."
  npm ci
fi

echo "Starting the Rust API at http://127.0.0.1:3000..."
ASCII_FONT_PATH="$ASCII_FONT_PATH" cargo run --bin server &
SERVER_PID=$!

cleanup() {
  echo
  echo "Stopping the Rust API..."
  kill "$SERVER_PID" 2>/dev/null || true
  wait "$SERVER_PID" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

sleep 1
if ! kill -0 "$SERVER_PID" 2>/dev/null; then
  wait "$SERVER_PID" 2>/dev/null || true
  echo "Error: the Rust API could not start. Is port 3000 already in use?" >&2
  exit 1
fi

echo "Starting the frontend..."
echo "Open the Vite URL shown below, usually http://localhost:5173"
npm run dev
