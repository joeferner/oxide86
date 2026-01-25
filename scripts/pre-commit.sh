#!/bin/bash
# Script to assemble and run 8086 assembly programs
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}/.."

cargo fmt
cargo build --all
cargo clippy --all
cargo test --all
