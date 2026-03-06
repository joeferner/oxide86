#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}/.."

./scripts/compile-examples.sh
cargo build --all
cargo fmt
cargo clippy --all -- -D warnings
./wasm/scripts/build.sh
cargo test --all

if command -v docker &> /dev/null; then
    echo ""
    echo "Building production Docker container..."
    docker build -t oxide86-web:latest ./wasm/www
else
    echo "Skipping docker build, docker not installed"
fi

echo ""
echo "Complete!"
