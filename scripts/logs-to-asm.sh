#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
exec cargo run -p oxide86-tools --bin logs-to-asm -- "$@"
