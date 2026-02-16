#!/bin/bash
# Script to assemble and run 8086 assembly programs
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}/.."

scripts/compile-test-programs.sh

cargo fmt
cargo build --all
cargo clippy --all -- -D warnings
./wasm/scripts/build.sh
cargo test --all

echo ""
echo "Building production Docker container..."
docker build -t emu86-web:latest ./wasm/www

echo ""
echo "Complete!"
