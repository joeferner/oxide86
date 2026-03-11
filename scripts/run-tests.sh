#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}/.."

find . -name '*_actual.png' -print0 | xargs -0 -r rm
cargo test --all
