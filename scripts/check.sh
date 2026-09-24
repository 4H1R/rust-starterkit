#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# A dedicated test database URL is mandatory. The suite creates/drops only random schemas.
: "${TEST_DATABASE_URL:?Set TEST_DATABASE_URL to a disposable PostgreSQL database}"
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
bash scripts/openapi.sh check
cargo deny --locked check
cargo build --locked --release --bin rust-starterkit
