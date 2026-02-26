#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}/.."

echo "Building oxide86 WASM module..."

# Build WASM with wasm-pack
wasm-pack build --target web --out-dir www/pkg

cd "${SCRIPT_DIR}/../www"
source "${HOME}/.nvm/nvm.sh"
nvm install
npm install --no-save
npm run pre-commit

echo ""
echo "WASM Build complete!"
