#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}/.."

cargo build --all
cargo fmt
cargo clippy --all -- -D warnings
./wasm/scripts/build.sh
cargo test --all

echo ""
echo "Building production Docker container..."
docker build -t oxide86-web:latest ./wasm/www

echo ""
echo "Complete!"
