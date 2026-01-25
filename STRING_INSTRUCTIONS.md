# 8086 String Instructions Implementation

This document describes the implementation of the 8086 string instructions in emu86.

## Overview

String instructions are a powerful feature of the 8086 processor that allow efficient manipulation of blocks of memory. These instructions automatically handle pointer increment/decrement and can be combined with the REP prefix (future enhancement) for repeated operations.

## Implemented Instructions

### Data Movement

#### MOVS - Move String (Opcodes: A4, A5)
- **MOVSB (A4)**: Move byte from DS:SI to ES:DI
- **MOVSW (A5)**: Move word from DS:SI to ES:DI

Copies data from source (DS:SI) to destination (ES:DI), then automatically increments or decrements both SI and DI based on the Direction Flag.

**Example:**
```asm
cld                 ; Clear direction flag (forward)
mov si, 0x100       ; Source address
mov di, 0x200       ; Destination address
movsb               ; Copy byte and increment SI, DI
```

#### LODS - Load String (Opcodes: AC, AD)
- **LODSB (AC)**: Load byte from DS:SI into AL
- **LODSW (AD)**: Load word from DS:SI into AX

Loads data from DS:SI into the accumulator (AL or AX), then updates SI.

**Example:**
```asm
mov si, 0x100
lodsb               ; AL = [DS:SI], SI++
```

#### STOS - Store String (Opcodes: AA, AB)
- **STOSB (AA)**: Store AL into byte at ES:DI
- **STOSW (AB)**: Store AX into word at ES:DI

Stores the accumulator value into ES:DI, then updates DI.

**Example:**
```asm
mov di, 0x100
mov al, 0x42
stosb               ; [ES:DI] = AL, DI++
```

### Comparison and Searching

#### CMPS - Compare String (Opcodes: A6, A7)
- **CMPSB (A6)**: Compare byte at DS:SI with byte at ES:DI
- **CMPSW (A7)**: Compare word at DS:SI with word at ES:DI

Compares source and destination by performing subtraction (DS:SI - ES:DI), sets flags accordingly, then updates both SI and DI. The result is not stored.

**Flags affected:** CF, ZF, SF, OF, AF, PF

**Example:**
```asm
mov si, string1
mov di, string2
cmpsb               ; Compare bytes, set flags
je equal            ; Jump if bytes are equal
```

#### SCAS - Scan String (Opcodes: AE, AF)
- **SCASB (AE)**: Compare AL with byte at ES:DI
- **SCASW (AF)**: Compare AX with word at ES:DI

Compares the accumulator with the value at ES:DI by performing subtraction (AL/AX - [ES:DI]), sets flags, then updates DI.

**Flags affected:** CF, ZF, SF, OF, AF, PF

**Example:**
```asm
mov di, buffer
mov al, 0x00        ; Search for null terminator
scasb
je found_null
```

### Direction Control

#### CLD - Clear Direction Flag (Opcode: FC)
Sets the Direction Flag to 0, causing string operations to increment SI/DI (forward/ascending direction).

**Example:**
```asm
cld                 ; Process strings forward
```

#### STD - Set Direction Flag (Opcode: FD)
Sets the Direction Flag to 1, causing string operations to decrement SI/DI (backward/descending direction).

**Example:**
```asm
std                 ; Process strings backward
```

## Direction Flag Behavior

The Direction Flag (DF) controls how SI and DI are updated after each string operation:

| DF | Direction | SI/DI Update | Use Case |
|----|-----------|--------------|----------|
| 0  | Forward   | Increment    | Normal copying, ascending addresses |
| 1  | Backward  | Decrement    | Overlapping moves, descending addresses |

**Increment/Decrement amounts:**
- Byte operations (MOVSB, STOSB, etc.): ±1
- Word operations (MOVSW, STOSW, etc.): ±2

## Flags Affected

### MOVS, LODS, STOS
These instructions do **not** affect any flags.

### CMPS, SCAS
These instructions affect flags as if a SUB instruction was performed:
- **CF (Carry)**: Set if borrow occurred (unsigned underflow)
- **ZF (Zero)**: Set if result is zero (operands are equal)
- **SF (Sign)**: Set if result is negative
- **OF (Overflow)**: Set if signed overflow occurred
- **AF (Auxiliary)**: Set if borrow from bit 3
- **PF (Parity)**: Set if low byte has even parity

## Implementation Details

### File Structure
- **[core/src/cpu/instructions/string.rs](core/src/cpu/instructions/string.rs)**: All string instruction implementations
- **[core/src/cpu/mod.rs](core/src/cpu/mod.rs)**: Opcode dispatch table

### Key Implementation Points

1. **Segment Registers**:
   - Source operations (MOVS, CMPS, LODS) use DS:SI (can be overridden with segment prefix)
   - Destination operations use ES:DI (cannot be overridden)

2. **Physical Address Calculation**:
   ```rust
   let addr = Self::physical_address(segment, offset);
   // Physical = (Segment × 16) + Offset
   ```

3. **Direction Flag Check**:
   ```rust
   if self.get_flag(FLAG_DIRECTION) {
       // Decrement
   } else {
       // Increment
   }
   ```

4. **Flag Setting for Comparisons**:
   - Separate helper functions for 8-bit and 16-bit subtraction
   - Properly calculates all arithmetic flags (CF, ZF, SF, OF, AF, PF)

## Examples

### Example 1: Memory Fill
See [examples/15-string_simple.asm](examples/15-string_simple.asm) for a basic example.

### Example 2: Comprehensive Test
See [examples/14-string_instructions.asm](examples/14-string_instructions.asm) for a comprehensive test of all string instructions.

## Future Enhancements

The following features are commonly used with string instructions but not yet implemented:

1. **REP Prefix (F3)**: Repeat while CX ≠ 0
   - Automatically decrements CX and repeats the operation
   - Used with MOVS, STOS, LODS

2. **REPE/REPZ Prefix (F3)**: Repeat while CX ≠ 0 and ZF = 1
   - Used with CMPS, SCAS for string comparison/searching

3. **REPNE/REPNZ Prefix (F2)**: Repeat while CX ≠ 0 and ZF = 0
   - Used with CMPS, SCAS for finding differences

Example of future usage:
```asm
; Copy 100 bytes
mov cx, 100
rep movsb           ; Repeat movsb 100 times

; Find null terminator
mov cx, 1000
mov al, 0
repne scasb         ; Scan until AL = [ES:DI] or CX = 0
```

## Testing

To test the string instructions:

```bash
# Build the emulator
cargo build --release

# Assemble an example
cd examples
nasm -f bin 15-string_simple.asm -o 15-string_simple.bin

# Run it (once the native runner is fully implemented)
../target/release/emu86-native 15-string_simple.bin
```

## References

- Intel 8086 Family User's Manual, Section 2.4: String Instructions
- [x86 Instruction Set Reference](https://www.felixcloutier.com/x86/)
- [8086 Opcode Map](http://www.mlsite.net/8086/)
