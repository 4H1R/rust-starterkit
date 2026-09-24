#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
docker_bin=${DOCKER_BIN:-docker}
case "${1:-help}" in
  up) "$docker_bin" compose up -d --wait ;;
  down) "$docker_bin" compose down ;;
  migrate|run)
    if [[ -f .env ]]; then set -a; source .env; set +a; fi
    if [[ "$1" == migrate ]]; then cargo run --locked --bin rust-starterkit -- migrate
    else cargo run --locked --bin rust-starterkit -- serve; fi ;;
  *) echo 'Usage: bash scripts/dev.sh {up|down|migrate|run}'; exit 2 ;;
esac
