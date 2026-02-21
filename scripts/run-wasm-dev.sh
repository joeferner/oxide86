#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}/../wasm/www"

echo "Starting oxide86 web development server..."
echo ""
echo "This will:"
echo "  1. Build the Docker development image"
echo "  2. Start the Vite dev server with live reload"
echo "  3. Mount source files for instant updates"
echo ""

# Check if WASM package exists
if [ ! -d "pkg" ] || [ ! -f "pkg/oxide86_wasm.js" ]; then
    echo "WARNING: WASM package not found in pkg/"
    echo "Please build the WASM package first:"
    echo ""
    echo "  cd ../../"
    echo "  sh scripts/build.sh"
    echo "  cd www"
    echo ""
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

echo "Starting development server..."
docker-compose up --build dev
