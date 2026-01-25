# emu86 - 8086 Emulator

An Intel 8086 CPU emulator written in Rust with support for both native execution and WebAssembly.

## Project Overview

emu86 is a software emulator for the Intel 8086 microprocessor, the 16-bit CPU that powered the original IBM PC. This project aims to accurately emulate the 8086's instruction set, registers, memory model, and behavior.

## Coding Rules

- Examples should be written in x86 assembly language.
- nasm should be assumed to be installed so no need to write special python assemblers.
- when compiling and running examples use examples/run.sh or update run.sh as needed
- avoid using python for any tasks
- don't write unit tests or integration tests unless directed to
- write code in core that supports both native and wasm
- don't support backwards compatibility
- when done run clippy
- when done update CLAUDE.md to help future code edits

## Architecture

The project is organized as a Rust workspace with three main components:

### Core Library ([core/](core/))
The `emu86-core` crate contains the main emulation logic:
- CPU state management (registers, flags)
- Instruction decoding and execution
- Memory management (segmented memory model)
- No platform-specific dependencies

### Native CLI ([native/](native/))
The `emu86-native` crate provides a command-line interface for running 8086 programs:
- Load and execute binary files
- Interactive debugging capabilities
- Memory inspection and register viewing

### WebAssembly Build ([wasm/](wasm/))
The `emu86-wasm` crate provides WebAssembly bindings:
- Browser-based emulation
- JavaScript API for integration
- Enables web-based 8086 development tools

## Key Features (Planned)

- **Complete 8086 Instruction Set**: Support for all documented 8086 instructions
- **Accurate CPU Emulation**: Flags, interrupts, and timing
- **Segmented Memory Model**: 1MB addressable memory space with segment registers (CS, DS, SS, ES)
- **Platform Agnostic Core**: Pure Rust implementation without platform dependencies
- **Dual Deployment**: Run as a native application or in the browser via WASM

## 8086 Architecture Reference

### Registers
- **General Purpose**: AX, BX, CX, DX (can be accessed as 16-bit or 8-bit: AH/AL, BH/BL, etc.)
- **Index Registers**: SI (Source Index), DI (Destination Index)
- **Pointer Registers**: SP (Stack Pointer), BP (Base Pointer)
- **Segment Registers**: CS (Code), DS (Data), SS (Stack), ES (Extra)
- **Instruction Pointer**: IP
- **Flags Register**: Carry, Parity, Auxiliary, Zero, Sign, Trap, Interrupt, Direction, Overflow

### Memory Addressing
- Segmented memory model: Physical Address = (Segment × 16) + Offset
- 20-bit address bus (1MB addressable memory)
- 16-bit data bus

### Interrupt Handling Architecture

**BIOS Interrupt Implementation**

The emulator supports BIOS interrupts through a trait-based system that allows platform-specific I/O while keeping the core platform-agnostic.

**Interrupt Dispatch Flow:**
```
INT instruction (0xCD/0xCC) → Computer::step() intercepts
  → Cpu::execute_int_with_io(int_num, memory, bios, video)
  → Cpu::handle_bios_interrupt(int_num, memory, bios, video)
  → Specific interrupt handler (handle_int10, handle_int13, handle_int21)
  → Individual service functions based on AH register
```

**Implemented Interrupts:**

- **INT 10h - Video Services** ([core/src/cpu/bios.rs](core/src/cpu/bios.rs))
  - AH=00h: Set video mode
  - AH=02h: Set cursor position
  - AH=06h/07h: Scroll up/down
  - AH=09h: Write character and attribute
  - AH=0Eh: Teletype output
  - AH=13h: Write string

- **INT 13h - Disk Services** ([core/src/cpu/bios.rs](core/src/cpu/bios.rs))
  - AH=00h: Reset disk system
  - AH=02h: Read sectors
  - AH=03h: Write sectors
  - AH=08h: Get drive parameters

- **INT 21h - DOS Services** ([core/src/cpu/bios.rs](core/src/cpu/bios.rs))
  - **Console I/O:**
    - AH=01h: Read character with echo
    - AH=02h: Write character
    - AH=09h: Write string
  - **Directory Operations:**
    - AH=39h: Create directory (MKDIR)
    - AH=3Ah: Remove directory (RMDIR)
    - AH=3Bh: Change directory (CHDIR)
    - AH=47h: Get current directory
    - AH=4Eh: Find first file
    - AH=4Fh: Find next file
  - **File Operations:**
    - AH=3Ch: Create or truncate file
    - AH=3Dh: Open existing file
    - AH=3Eh: Close file
    - AH=3Fh: Read from file or device
    - AH=40h: Write to file or device
    - AH=42h: Seek (LSEEK)
  - **Process Control:**
    - AH=4Ch: Exit program
  - **System Functions:**
    - AH=19h: Get current default drive
    - AH=25h: Set interrupt vector
    - AH=30h: Get DOS version
    - AH=35h: Get interrupt vector

**Adding New BIOS Interrupts:**

To add a new BIOS interrupt handler:

1. Add interrupt handler method in [core/src/cpu/bios.rs](core/src/cpu/bios.rs):
   ```rust
   fn handle_intXX(&mut self, memory: &mut Memory, io: &mut T, video: &mut Video) {
       let function = (self.ax >> 8) as u8; // Get AH
       match function {
           0x00 => self.intXX_function_00(...),
           _ => warn!("Unhandled INT 0xXX function: AH=0x{:02X}", function),
       }
   }
   ```

2. Add case to `handle_bios_interrupt()` dispatch in the same file:
   ```rust
   match int_num {
       0xXX => {
           self.handle_intXX(memory, io, video);
           true
       }
       // ... existing cases
   }
   ```

3. No changes needed to Computer or CPU core - dispatch is automatic

**DOS File Operations (INT 21h):**

The emulator implements DOS file operations through the `Bios` trait. Platform-specific implementations (native, WASM) must provide actual file I/O.

**File Operation Flow:**
```
INT 21h with AH=3Ch-42h → handle_int21() → int21_*_file()
  → Bios trait method (file_create, file_open, etc.)
  → Platform-specific implementation
  → Return file handle or error code
```

**DOS Error Codes** (defined in `dos_errors` module):
- `SUCCESS` (0x00): Operation successful
- `INVALID_FUNCTION` (0x01): Invalid function number
- `FILE_NOT_FOUND` (0x02): File not found
- `PATH_NOT_FOUND` (0x03): Path not found
- `TOO_MANY_OPEN_FILES` (0x04): No handles available
- `ACCESS_DENIED` (0x05): Permission denied
- `INVALID_HANDLE` (0x06): Invalid file handle
- `INVALID_DRIVE` (0x0F): Invalid drive specification
- `ATTEMPT_TO_REMOVE_CURRENT_DIR` (0x10): Cannot remove current directory
- `NO_MORE_FILES` (0x12): No more matching files in directory search

**File Access Modes** (for AH=3Dh - Open File):
- `READ_ONLY` (0x00): Open for reading only
- `WRITE_ONLY` (0x01): Open for writing only
- `READ_WRITE` (0x02): Open for both reading and writing

**Seek Methods** (for AH=42h - LSEEK):
- `SeekMethod::FromStart` (0): Seek from beginning of file
- `SeekMethod::FromCurrent` (1): Seek from current position
- `SeekMethod::FromEnd` (2): Seek from end of file

**File Attributes** (defined in `file_attributes` module):
- `READ_ONLY` (0x01): File is read-only
- `HIDDEN` (0x02): File is hidden
- `SYSTEM` (0x04): File is a system file
- `VOLUME_LABEL` (0x08): Entry is a volume label
- `DIRECTORY` (0x10): Entry is a directory
- `ARCHIVE` (0x20): File has been modified since last backup

**Implementing File Operations in Platform Code:**

To support file operations, implement these `Bios` trait methods:

```rust
fn file_create(&mut self, filename: &str, attributes: u8) -> Result<u16, u8>;
fn file_open(&mut self, filename: &str, access_mode: u8) -> Result<u16, u8>;
fn file_close(&mut self, handle: u16) -> Result<(), u8>;
fn file_read(&mut self, handle: u16, max_bytes: u16) -> Result<Vec<u8>, u8>;
fn file_write(&mut self, handle: u16, data: &[u8]) -> Result<u16, u8>;
fn file_seek(&mut self, handle: u16, offset: i32, method: SeekMethod) -> Result<u32, u8>;
```

**Standard File Handles:**
- Handle 0: Standard Input (STDIN)
- Handle 1: Standard Output (STDOUT)
- Handle 2: Standard Error (STDERR)

Platform implementations should reserve these handles for console I/O and start user file handles at 3 or higher.

**DOS Directory Operations (INT 21h):**

The emulator implements DOS directory operations through the `Bios` trait. Platform-specific implementations must provide actual directory manipulation.

**Directory Operation Flow:**
```
INT 21h with AH=39h-4Fh → handle_int21() → int21_*_dir() / int21_find_*()
  → Bios trait method (dir_create, dir_remove, find_first, etc.)
  → Platform-specific implementation
  → Return success or error code
```

**Implementing Directory Operations in Platform Code:**

To support directory operations, implement these `Bios` trait methods:

```rust
fn dir_create(&mut self, dirname: &str) -> Result<(), u8>;
fn dir_remove(&mut self, dirname: &str) -> Result<(), u8>;
fn dir_change(&mut self, dirname: &str) -> Result<(), u8>;
fn dir_get_current(&self, drive: u8) -> Result<String, u8>;
fn find_first(&mut self, pattern: &str, attributes: u8) -> Result<(usize, FindData), u8>;
fn find_next(&mut self, search_id: usize) -> Result<FindData, u8>;
```

**FindData Structure:**

File search operations return a `FindData` structure containing:
- `attributes`: File attributes (see File Attributes above)
- `time`: File modification time in DOS packed format (bits 0-4: seconds/2, 5-10: minutes, 11-15: hours)
- `date`: File modification date in DOS packed format (bits 0-4: day, 5-8: month, 9-15: year-1980)
- `size`: File size in bytes (32-bit)
- `filename`: Filename in null-terminated string format (max 13 bytes for 8.3 format)

**DTA (Disk Transfer Area) Format:**

Find first/next operations use a 43-byte DTA structure:
- Offset 0-20: Reserved for internal use (search state)
- Offset 21: File attributes (1 byte)
- Offset 22-23: File time (2 bytes, little-endian)
- Offset 24-25: File date (2 bytes, little-endian)
- Offset 26-29: File size (4 bytes, little-endian)
- Offset 30-42: Filename (null-terminated, max 13 bytes)

**Wildcard Matching:**

The native implementation supports DOS-style wildcards in file patterns:
- `*` matches any sequence of characters
- `?` matches any single character
- Pattern matching is case-insensitive

**Working Directory:**

The native implementation supports a working directory for file operations:
- All file paths are resolved relative to the working directory
- Absolute paths are used as-is
- The working directory can be specified with `--workdir <path>` command-line option
- Default working directory is the current directory
- The `examples/run.sh` script automatically creates and uses `examples/workdir/`
- `examples/workdir/` is ignored by git to avoid polluting the repository

**DOS System Functions (INT 21h):**

The emulator implements additional DOS system functions for compatibility with DOS programs:

- **AH=19h - Get Current Default Drive:**
  - Output: AL = current drive number (0=A, 1=B, etc.)
  - In the native implementation, always returns 0 (drive A) as Unix-like systems don't have drive letters

- **AH=25h - Set Interrupt Vector:**
  - Input: AL = interrupt number, DS:DX = new interrupt handler address
  - Updates the interrupt vector table (IVT) at address 0000:0000
  - Each IVT entry is 4 bytes: offset (2 bytes) + segment (2 bytes)
  - Used by programs to install custom interrupt handlers

- **AH=30h - Get DOS Version:**
  - Output: AL = major version, AH = minor version, BL:CX = serial number (usually 0)
  - Returns DOS 3.30 for compatibility with most DOS programs
  - DOS 3.30 is a well-supported version with good compatibility

- **AH=35h - Get Interrupt Vector:**
  - Input: AL = interrupt number
  - Output: ES:BX = current interrupt handler address
  - Reads the interrupt vector table (IVT) at address 0000:0000
  - Used by programs to save and restore interrupt vectors

**Interrupt Vector Table (IVT):**

The IVT is located at memory address 0000:0000 and contains 256 entries (one for each possible interrupt):
- Each entry is 4 bytes: 2-byte offset followed by 2-byte segment
- Entry for interrupt N is at address N * 4
- The IVT occupies the first 1KB of memory (0x0000-0x03FF)
- Programs can read and modify interrupt vectors using INT 21h functions 25h and 35h

**Critical Files:**
- [core/src/cpu/bios.rs](core/src/cpu/bios.rs) - BIOS interrupt handlers
- [core/src/cpu/mod.rs](core/src/cpu/mod.rs) - Interrupt dispatch (`execute_int_with_io`)
- [core/src/lib.rs](core/src/lib.rs) - Computer integration (INT opcode detection)
- [core/src/cpu/instructions/control_flow.rs](core/src/cpu/instructions/control_flow.rs) - INT instruction implementation

## Development

### Building

Build all workspace members:
```bash
cargo build
```

Build specific crate:
```bash
cargo build -p emu86-core
cargo build -p emu86-native
cargo build -p emu86-wasm
```

### Testing

Run tests:
```bash
cargo test
```

### Running

Native CLI:
```bash
cargo run -p emu86-native -- <args>
```

## Project Structure

```
emu86/
├── core/           # Platform-independent emulator core
├── examples/       # Example programs to run
├── native/         # Native CLI application
├── wasm/           # WebAssembly bindings
└── Cargo.toml      # Workspace configuration
```

## Implementation Guide

When implementing the emulator, consider this order:

1. **Core Data Structures** ([core/src/lib.rs](core/src/lib.rs))
   - CPU state (registers, flags)
   - Memory representation
   - Instruction structure

2. **Instruction Decoder**
   - Parse 8086 machine code
   - Handle various addressing modes
   - Support for prefixes

3. **Instruction Executor**
   - Implement instructions by category:
     - Data Transfer (MOV, PUSH, POP, etc.)
     - Arithmetic (ADD, SUB, MUL, DIV, etc.)
     - Logic (AND, OR, XOR, NOT, etc.)
     - Control Flow (JMP, CALL, RET, conditional jumps)
     - String Operations (MOVS, CMPS, SCAS, etc.)
     - Bit Manipulation (SHL, SHR, ROL, ROR, etc.)

4. **Native Interface** ([native/src/main.rs](native/src/main.rs))
   - Command-line argument parsing
   - Binary file loading
   - Debugging interface

5. **WASM Bindings** ([wasm/src/lib.rs](wasm/src/lib.rs))
   - JavaScript-friendly API
   - Memory inspection methods
   - Step execution for debugging

## Resources

- [Intel 8086 Family User's Manual](https://edge.edx.org/c4x/BITSPilani/EEE231/asset/8086_family_Users_Manual_1_.pdf)
- [x86 Instruction Set Reference](https://www.felixcloutier.com/x86/)
- [8086 Opcode Table](http://www.mlsite.net/8086/)
