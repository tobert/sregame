#!/usr/bin/env bash
# Compile gate for the browser build: keeps native-only dependencies (tokio,
# OTLP/tonic, BRP, std::fs, std::time::Instant) from creeping back into the
# universal code paths. Run it like the tests - red means a regression.
#
#   ./scripts/check-wasm.sh
#
# Requires: rustup target add wasm32-unknown-unknown
set -euo pipefail
cd "$(dirname "$0")/.."

exec cargo check --target wasm32-unknown-unknown "$@"
