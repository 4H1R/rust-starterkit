#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
case "${1:-check}" in
  write) cargo run --quiet --locked --bin openapi > docs/openapi.json ;;
  check)
    generated=$(mktemp)
    trap 'rm -f "$generated"' EXIT
    cargo run --quiet --locked --bin openapi > "$generated"
    diff -u docs/openapi.json "$generated" ;;
  *) echo 'Usage: bash scripts/openapi.sh {write|check}'; exit 2 ;;
esac
