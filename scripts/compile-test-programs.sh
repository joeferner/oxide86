#!/bin/bash

# Script to compile all .asm files in test-programs/ to .com files

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "Compiling test programs..."
echo

compiled=0
failed=0
skipped=0

# Find all .asm files in test-programs directory
while IFS= read -r asm_file; do
    # Get the directory and filename without extension
    dir=$(dirname "$asm_file")
    base=$(basename "$asm_file" .asm)
    com_file="$dir/$base.com"

    # Check if .com file exists and is newer than .asm file
    if [ -f "$com_file" ] && [ "$com_file" -nt "$asm_file" ]; then
        echo -e "${YELLOW}SKIP${NC} $asm_file (up to date)"
        ((skipped++))
        continue
    fi

    # Compile with nasm
    echo -n "Compiling $asm_file... "
    if nasm -f bin -o "$com_file" "$asm_file" 2>&1; then
        echo -e "${GREEN}OK${NC}"
        ((compiled++))
    else
        echo -e "${RED}FAILED${NC}"
        ((failed++))
    fi
done < <(find test-programs -name "*.asm" -type f | sort)

echo
echo "================================"
echo "Compiled: $compiled"
echo "Skipped:  $skipped (up to date)"
echo "Failed:   $failed"
echo "================================"

if [ $failed -gt 0 ]; then
    exit 1
fi
