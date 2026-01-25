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
- run clippy when done

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
  - AH=01h: Read character with echo
  - AH=02h: Write character
  - AH=09h: Write string
  - AH=4Ch: Exit program

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
