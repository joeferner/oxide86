#!/bin/bash
# Script to assemble and run 8086 assembly programs
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [ $# -lt 1 ]; then
    echo "Usage: $0 <file.asm> [--segment SEGMENT] [--offset OFFSET]"
    echo ""
    echo "Examples:"
    echo "  $0 00-simple.asm"
    echo "  $0 00-simple.asm --segment 0x1000 --offset 0x0000"
    exit 1
fi

ASM_FILE="$1"
shift

# Check if the file exists
if [ ! -f "$ASM_FILE" ]; then
    echo "Error: File '$ASM_FILE' not found"
    exit 1
fi

# Get the base filename without extension
BASENAME="${ASM_FILE%.asm}"
BIN_FILE="${BASENAME}.bin"

# Assemble the program
echo "Assembling $ASM_FILE..."
nasm -f bin "$ASM_FILE" -o "$BIN_FILE"

if [ $? -eq 0 ]; then
    echo "Successfully assembled to $BIN_FILE"
    echo ""

    # Run the program
    echo "Running $BIN_FILE..."
    echo "================================"
    cargo run -p emu86-native-cli -- "$BIN_FILE" "$@"
else
    echo "Assembly failed"
    exit 1
fi
