#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}/../examples"

for f in *.asm; do
    nasm -f bin -o "${f%.*}.com" "${f}"
done

echo ""
echo "Compile Examples Complete!"
