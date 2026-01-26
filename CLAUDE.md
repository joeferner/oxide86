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

**Native BIOS Implementation** ([native/src/bios/](native/src/bios/)):
The native platform implements the `Bios` trait through `NativeBios`, which is organized into focused modules:
- [native_bios.rs](native/src/bios/native_bios.rs) - Main `NativeBios` struct that implements the `Bios` trait
- [console.rs](native/src/bios/console.rs) - Console I/O operations (keyboard and screen)
- [disk.rs](native/src/bios/disk.rs) - Disk controller operations
- [file.rs](native/src/bios/file.rs) - File operations via `FileManager`
- [directory.rs](native/src/bios/directory.rs) - Directory operations via `DirectoryManager`
- [memory_allocator.rs](native/src/bios/memory_allocator.rs) - DOS memory allocation
- [time.rs](native/src/bios/time.rs) - System time and RTC operations
- [peripheral.rs](native/src/bios/peripheral.rs) - Serial port and printer stubs

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

**Flag Manipulation:**

The CPU provides a `set_flag` method for manipulating individual flags:
```rust
pub(super) fn set_flag(&mut self, flag: u16, value: bool)
```

Usage:
- To set a flag: `self.set_flag(cpu_flag::CARRY, true);`
- To clear a flag: `self.set_flag(cpu_flag::CARRY, false);`

**Important:** There is no `clear_flag` method. Always use `set_flag` with `false` to clear a flag.

### Memory Addressing
- Segmented memory model: Physical Address = (Segment × 16) + Offset
- 20-bit address bus (1MB addressable memory)
- 16-bit data bus

### Segment Override Prefixes

The emulator supports segment override prefixes for memory access instructions:
- **0x26**: ES override - Forces memory access to use ES segment
- **0x2E**: CS override - Forces memory access to use CS segment
- **0x36**: SS override - Forces memory access to use SS segment
- **0x3E**: DS override - Forces memory access to use DS segment

**Implementation** ([core/src/cpu/mod.rs](core/src/cpu/mod.rs)):
- The CPU maintains a `segment_override: Option<u16>` field
- When a segment override prefix is encountered, it sets `segment_override` to the appropriate segment register value
- The next instruction is executed recursively with the override active
- After execution, `segment_override` is cleared
- The following functions check `segment_override` and use it instead of the default segment:
  - `decode_modrm()` - For memory operands ([core/src/cpu/mod.rs](core/src/cpu/mod.rs))
  - `mov_acc_moffs()` - For direct memory offset moves ([core/src/cpu/instructions/data_transfer.rs](core/src/cpu/instructions/data_transfer.rs))
  - `xlat()` - For table lookup translation ([core/src/cpu/instructions/data_transfer.rs](core/src/cpu/instructions/data_transfer.rs))
  - String instruction source operands (DS:SI) ([core/src/cpu/instructions/string.rs](core/src/cpu/instructions/string.rs)):
    - `movs()` - MOVSB/MOVSW source
    - `cmps()` - CMPSB/CMPSW source
    - `lods()` - LODSB/LODSW
    - `outs()` - OUTSB/OUTSW

**Important Notes:**
- String instruction destinations (ES:DI) **cannot** be overridden - ES is hardcoded per x86 spec
- Stack operations (SS:SP) do not use segment overrides
- Code fetching (CS:IP) does not use segment overrides
- BIOS/DOS interrupt handlers use specific segments as defined by their API contracts

**Example Usage:**
```asm
mov ax, 0x0040
mov es, ax
mov ax, [es:0x10]  ; Read from ES:0x0010 instead of DS:0x0010

; String instruction with override
mov ax, 0x0040
mov es, ax
lodsb              ; Normally loads from DS:SI
es lodsb           ; With override, loads from ES:SI
```

### String Instruction Repeat Prefixes

The emulator supports repeat prefixes for string instructions:
- **0xF2**: REPNE/REPNZ - Repeat while CX ≠ 0 and ZF = 0 (not equal)
- **0xF3**: REP/REPE/REPZ - Repeat while CX ≠ 0 (and ZF = 1 for conditional variants)

**Implementation** ([core/src/cpu/mod.rs](core/src/cpu/mod.rs), [core/src/cpu/instructions/string.rs](core/src/cpu/instructions/string.rs)):
- The CPU maintains a `repeat_prefix: Option<RepeatPrefix>` field
- When a repeat prefix is encountered, it sets `repeat_prefix` and executes the next instruction
- After execution, `repeat_prefix` is cleared
- String instructions check for the repeat prefix and loop accordingly:
  - **Simple REP** (MOVS, STOS, LODS, INS, OUTS): Repeat while CX ≠ 0
  - **REPE/REPZ** (CMPS, SCAS): Repeat while CX ≠ 0 and ZF = 1, stop when ZF = 0
  - **REPNE/REPNZ** (CMPS, SCAS): Repeat while CX ≠ 0 and ZF = 0, stop when ZF = 1
- Each iteration decrements CX

**Supported String Instructions:**
- `MOVS` (A4-A5) - Move String
- `CMPS` (A6-A7) - Compare String (with REPE/REPNE)
- `SCAS` (AE-AF) - Scan String (with REPE/REPNE)
- `LODS` (AC-AD) - Load String
- `STOS` (AA-AB) - Store String
- `INS` (6C-6D) - Input String from Port
- `OUTS` (6E-6F) - Output String to Port

**Example Usage:**
```asm
; Fill 100 bytes with 0x00
mov cx, 100
mov di, 0x1000
xor al, al
cld                ; Direction = forward
rep stosb          ; Repeat STOSB while CX != 0

; Copy 50 words from DS:SI to ES:DI
mov cx, 50
mov si, 0x2000
mov di, 0x3000
cld
rep movsw          ; Repeat MOVSW while CX != 0

; Find first non-zero byte in buffer
mov cx, 1000
mov di, 0x4000
xor al, al
cld
repe scasb         ; Repeat while CX != 0 and byte == 0
; DI now points to first non-zero byte (or end if all zero)

; Find null terminator in string
mov di, string_offset
mov al, 0
mov cx, 0xFFFF     ; Max length
cld
repne scasb        ; Repeat while CX != 0 and byte != 0
; DI now points past the null terminator
```

### BIOS Data Area (BDA)

The emulator initializes a BIOS Data Area at segment 0x0040 (physical address 0x0400) containing system configuration information, compatible with IBM PC BIOS.

**Initialization** ([core/src/memory.rs](core/src/memory.rs)):
- The BDA is automatically initialized by `Memory::initialize_bda()` when the `Computer` is created
- Total size: 256 bytes (0x0040:0000 to 0x0040:00FF)

**BDA Fields:**

| Offset | Size | Description | Default Value |
|--------|------|-------------|---------------|
| 0x00 | 8 bytes | COM1-COM4 port addresses | 0x03F8, 0x02F8, 0x03E8, 0x02E8 |
| 0x08 | 8 bytes | LPT1-LPT4 port addresses | 0x0378, 0x0278, 0x03BC, 0x0000 |
| 0x10 | 2 bytes | Equipment list word | 0x0061 (floppy + color 80x25) |
| 0x13 | 2 bytes | Memory size in KB | 640 |
| 0x17 | 1 byte | Keyboard shift flags 1 | 0x00 |
| 0x18 | 1 byte | Keyboard shift flags 2 | 0x00 |
| 0x1A | 2 bytes | Keyboard buffer head pointer | 0x001E |
| 0x1C | 2 bytes | Keyboard buffer tail pointer | 0x001E |
| 0x1E | 32 bytes | Keyboard buffer (16 scan code/char pairs) | All zeros |
| 0x49 | 1 byte | Current video mode | 0x03 (80x25 color text) |
| 0x4A | 2 bytes | Number of screen columns | 80 |
| 0x4C | 2 bytes | Video page size in bytes | 4000 |
| 0x4E | 2 bytes | Current video page offset | 0x0000 |
| 0x50 | 16 bytes | Cursor positions for 8 pages | All zeros (row 0, col 0) |
| 0x60 | 1 byte | Cursor end scan line | 0x0D |
| 0x61 | 1 byte | Cursor start scan line | 0x0C |
| 0x62 | 1 byte | Active display page | 0x00 |
| 0x63 | 2 bytes | CRT controller base port | 0x03D4 (color) |
| 0x65 | 1 byte | CRT mode control register | 0x09 |
| 0x66 | 1 byte | CRT palette register | 0x00 |
| 0x6C | 4 bytes | Timer counter (ticks since midnight) | 0x00000000 |
| 0x70 | 1 byte | Timer midnight rollover flag | 0x00 |

**Equipment List Bits** (offset 0x10):
- Bit 0: Floppy drive installed
- Bits 4-5: Initial video mode (0x20 = 80x25 color, 0x30 = 80x25 mono)
- Bits 6-7: Number of floppy drives minus 1
- Bits 9-11: Number of serial ports
- Bits 14-15: Number of printers

**Reading BDA in Assembly:**
```asm
; Set ES to BDA segment
mov ax, 0x0040
mov es, ax

; Read equipment list
mov ax, [es:0x10]

; Read memory size in KB
mov ax, [es:0x13]

; Read video mode
mov al, [es:0x49]
```

**Example:** See [examples/bda_test.asm](examples/bda_test.asm) for a complete BDA reading example.

**Keyboard Buffer:**

The keyboard buffer (offset 0x1E, 32 bytes) is used by INT 16h keyboard services:
- Stores up to 16 keystrokes as scan code/ASCII pairs (2 bytes each)
- Head pointer (0x1A) indicates next keystroke to read
- Tail pointer (0x1C) indicates next free slot to write
- When head == tail, buffer is empty
- Buffer operates as a circular queue (wraps from 0x003D to 0x001E)
- INT 16h AH=00h reads and removes from buffer
- INT 16h AH=01h peeks without removing

**System Timer:**

The system timer counter (offset 0x6C, 4 bytes) tracks time using the PIT (Programmable Interval Timer):
- Counts clock ticks since midnight at 18.2 Hz (approximately every 54.925 ms)
- Maximum value: 0x001800B0 (1,573,040 ticks = 24 hours)
- Timer overflow flag (offset 0x70) indicates if midnight has passed since last read
- INT 1Ah AH=00h reads the tick count and returns the midnight flag (then clears it)
- INT 1Ah AH=01h sets the tick count and clears the midnight flag
- The timer counter is a 32-bit value stored in little-endian format (low word at 0x6C, high word at 0x6E)

**Timer Emulation:**

The emulator simulates the PIT timer with cycle-based timing:
- **Initialization**: On startup, the timer is initialized from the host system time via `Bios::get_system_ticks()`
  - Native implementation reads the current time of day and converts to BIOS ticks
  - This ensures programs see a realistic time value immediately
- **Automatic Increment**: The timer automatically increments as the CPU executes instructions
  - Each instruction execution adds to a cycle counter
  - When the cycle count reaches ~262,088 cycles (simulating 18.2 Hz on a 4.77 MHz 8086), the timer increments by 1 tick
  - This happens in `Computer::increment_cycles()` which is called after each instruction in `Computer::step()`
- **Midnight Rollover**: When the timer reaches 0x001800B0 (24 hours), it:
  - Resets to 0
  - Sets the midnight overflow flag (BDA offset 0x70) to 1
  - The flag is cleared when read by INT 1Ah AH=00h
- **Manual Control**: Programs can still set the timer to any value using INT 1Ah AH=01h
  - The cycle-based increment continues from the new value
  - This allows programs to synchronize or adjust the time

**Reading Timer in Assembly:**
```asm
; Get system time using INT 1Ah
mov ah, 0x00           ; Function 00h - Get system time
int 0x1A               ; Call time service
; CX:DX now contains tick count
; AL contains midnight flag (non-zero if midnight passed)

; Set system time
mov cx, 0x0001         ; High word of tick count
mov dx, 0x2345         ; Low word of tick count
mov ah, 0x01           ; Function 01h - Set system time
int 0x1A               ; Call time service
```

**Example:** See [examples/time_test.asm](examples/time_test.asm) for a complete timer test example.

**Real Time Clock (RTC):**

The emulator supports reading the real-time clock via INT 1Ah AH=02h:
- **Platform Support**: Available on AT-class systems (286+), not available on original 8086/XT
  - Native implementation reads the host system time and returns current time
  - NullBios implementation returns error (CF=1) to indicate RTC not present
- **BCD Format**: Time values are returned in Binary Coded Decimal format
  - Each decimal digit is stored in 4 bits: 23 decimal = 0x23 BCD
  - Hours: 0-23, Minutes: 0-59, Seconds: 0-59
- **Daylight Saving Time**: DL register contains DST flag (0 = standard time, 1 = daylight time)
  - Native implementation currently returns 0 (no DST support)

**Reading RTC in Assembly:**
```asm
; Read current time from RTC using INT 1Ah AH=02h
mov ah, 0x02           ; Function 02h - Read RTC time
int 0x1A               ; Call time service
jc rtc_not_available   ; CF=1 if RTC not present or not operating

; Success - time is returned in BCD format
; CH = hours (BCD, 0-23)
; CL = minutes (BCD, 0-59)
; DH = seconds (BCD, 0-59)
; DL = daylight saving time flag

; Convert BCD hours to decimal (if needed)
mov al, ch             ; AL = hours in BCD (e.g., 0x23 = 23)
mov bl, al
and al, 0x0F           ; AL = low digit (ones)
shr bl, 4              ; BL = high digit (tens)
mov cl, 10
mul cl                 ; AX = tens * 10
add al, bl             ; AL = decimal hours

rtc_not_available:
; Handle RTC not available (8086/XT systems)
```

**Implementation Details:**

The RTC is implemented through the `Bios` trait:
- `get_rtc_time() -> Option<RtcTime>`: Platform implementations return current time or None
  - Returns `RtcTime` struct with hours, minutes, seconds, dst_flag in decimal format
  - INT 1Ah handler converts decimal to BCD before returning to caller
- **Native Platform**: Reads host system time via `SystemTime::now()` and extracts time of day
- **Null BIOS**: Returns None to indicate RTC not available (simulates 8086/XT behavior)

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

- **INT 10h - Video Services** ([core/src/cpu/bios/int10.rs](core/src/cpu/bios/int10.rs))
  - AH=00h: Set video mode
  - AH=02h: Set cursor position
  - AH=06h/07h: Scroll up/down
  - AH=09h: Write character and attribute
  - AH=0Eh: Teletype output
  - AH=13h: Write string

- **INT 12h - Get Memory Size** ([core/src/cpu/bios/int12.rs](core/src/cpu/bios/int12.rs))
  - Returns AX = conventional memory size in KB (typically 640)
  - No function codes - directly returns memory size from BDA

- **INT 13h - Disk Services** ([core/src/cpu/bios/int13.rs](core/src/cpu/bios/int13.rs))
  - AH=00h: Reset disk system
  - AH=02h: Read sectors
  - AH=03h: Write sectors
  - AH=08h: Get drive parameters
  - AH=15h: Get disk type

- **INT 14h - Serial Port Services** ([core/src/cpu/bios/int14.rs](core/src/cpu/bios/int14.rs))
  - AH=00h: Initialize serial port - Configure baud rate, parity, stop bits, and word length
  - AH=01h: Write character - Transmit a character to the serial port
  - AH=02h: Read character - Receive a character from the serial port
  - AH=03h: Get status - Get line and modem status

- **INT 15h - Miscellaneous System Services** ([core/src/cpu/bios/int15.rs](core/src/cpu/bios/int15.rs))
  - AH=41h: Wait for external event - PS/2 function (returns not supported on 8086)
  - AH=86h: Wait - Microsecond delay (returns immediately in emulator)
  - AH=88h: Get extended memory size - Returns 0 KB for 8086 (no extended memory)
  - AH=C0h: Get system configuration - Returns pointer to system descriptor table

- **INT 16h - Keyboard Services** ([core/src/cpu/bios/int16.rs](core/src/cpu/bios/int16.rs))
  - AH=00h: Read character - Waits for a keypress and returns scan code in AH and ASCII in AL
  - AH=01h: Check for keystroke - Checks if a key is available without removing it (sets ZF if none)
  - AH=02h: Get shift flags - Returns keyboard shift/lock state in AL

- **INT 17h - Printer Services** ([core/src/cpu/bios/int17.rs](core/src/cpu/bios/int17.rs))
  - AH=00h: Print character - Send a character to the printer
  - AH=01h: Initialize printer port - Reset and initialize the printer
  - AH=02h: Get printer status - Read the current printer status

- **INT 1Ah - Time Services** ([core/src/cpu/bios/int1a.rs](core/src/cpu/bios/int1a.rs))
  - AH=00h: Get system time - Returns CX:DX = tick count since midnight, AL = midnight flag
  - AH=01h: Set system time - Sets timer counter to CX:DX (ticks since midnight)
  - AH=02h: Read RTC time - Returns CH = hours (BCD), CL = minutes (BCD), DH = seconds (BCD), DL = DST flag, CF = 0 on success

- **INT 21h - DOS Services** ([core/src/cpu/bios/int21.rs](core/src/cpu/bios/int21.rs))
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
    - AH=44h: IOCTL (Input/Output Control)
    - AH=45h: Duplicate file handle
  - **Memory Management:**
    - AH=48h: Allocate memory
    - AH=49h: Free memory
    - AH=4Ah: Resize memory block
  - **Process Control:**
    - AH=4Ch: Exit program
    - AH=50h: Set PSP address
  - **System Functions:**
    - AH=0Eh: Select default disk
    - AH=19h: Get current default drive
    - AH=25h: Set interrupt vector
    - AH=30h: Get DOS version
    - AH=35h: Get interrupt vector
    - AH=37h: Get/Set switch character (obsolete, returns not supported)

- **INT 29h - Fast Console Output** ([core/src/cpu/bios/int29.rs](core/src/cpu/bios/int29.rs))
  - Faster character output for DOS
  - Input: AL = character to output
  - No function codes - directly outputs the character in AL
  - Used by DOS internally for console output

- **INT 2Fh - DOS Multiplex Interrupt** ([core/src/cpu/bios/int2f.rs](core/src/cpu/bios/int2f.rs))
  - Inter-program communication and TSR installation checks
  - AH = multiplex number (function group), AL = subfunction
  - **Implemented multiplex numbers:**
    - AH=11h: Network redirector installation check - Returns AL=0x00 (not installed)
    - AH=12h: DOS internal functions (SHARE, PRINT) - Returns AL=0x00 (not installed)
    - AH=16h: Windows enhanced mode installation check - Returns AL=0x00 (not running)
    - AH=43h: XMS (Extended Memory Specification) - Returns AL=0x00 (not installed, 8086 has no extended memory)
    - AH=4Ah: HMA (High Memory Area) query - AL=00h: installation check (returns AL=0x00, not installed), AL=02h: release HMA (returns AL=0x00, not allocated/not supported on 8086)
    - AH=B7h: APPEND installation check - Returns AL=0x00 (not installed)
  - All unrecognized multiplex numbers return AL=0x00 (standard "not installed" response)

**Adding New BIOS Interrupts:**

To add a new BIOS interrupt handler:

1. Create a new file [core/src/cpu/bios/intXX.rs](core/src/cpu/bios/intXX.rs) with interrupt handler methods:
   ```rust
   use log::warn;
   use crate::{cpu::Cpu, memory::Memory};

   impl Cpu {
       pub(super) fn handle_intXX<T: super::Bios>(&mut self, memory: &mut Memory, io: &mut T) {
           let function = (self.ax >> 8) as u8; // Get AH
           match function {
               0x00 => self.intXX_function_00(...),
               _ => warn!("Unhandled INT 0xXX function: AH=0x{:02X}", function),
           }
       }
   }
   ```

2. Add module declaration in [core/src/cpu/bios/mod.rs](core/src/cpu/bios/mod.rs):
   ```rust
   mod intXX;
   ```

3. Add case to `handle_bios_interrupt()` dispatch in [core/src/cpu/bios/mod.rs](core/src/cpu/bios/mod.rs):
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

**Serial Port Services (INT 14h):**

The emulator implements BIOS serial port services through the `Bios` trait. Serial port operations allow communication with COM ports (COM1-COM4).

**Serial Port Parameters:**

When initializing a serial port (AH=00h), the AL register contains configuration parameters:
- Bits 7-5: Baud rate (000=110, 001=150, 010=300, 011=600, 100=1200, 101=2400, 110=4800, 111=9600)
- Bits 4-3: Parity (00=none, 01=odd, 10=none, 11=even)
- Bit 2: Stop bits (0=1 stop bit, 1=2 stop bits)
- Bits 1-0: Word length (10=7 bits, 11=8 bits)

**Line Status Bits (returned in AH):**
- Bit 7: Timeout
- Bit 6: Transmit shift register empty
- Bit 5: Transmit holding register empty
- Bit 4: Break detect
- Bit 3: Framing error
- Bit 2: Parity error
- Bit 1: Overrun error
- Bit 0: Data ready

**Modem Status Bits (returned in AL):**
- Bit 7: Received line signal detect
- Bit 6: Ring indicator
- Bit 5: Data set ready
- Bit 4: Clear to send
- Bit 3: Change in receive line signal detect
- Bit 2: Trailing edge ring indicator
- Bit 1: Change in data set ready
- Bit 0: Change in clear to send

**Implementing Serial Port Operations in Platform Code:**

To support serial ports, implement these `Bios` trait methods:

```rust
fn serial_init(&mut self, port: u8, params: SerialParams) -> SerialStatus;
fn serial_write(&mut self, port: u8, ch: u8) -> u8;
fn serial_read(&mut self, port: u8) -> Result<(u8, u8), u8>;
fn serial_status(&self, port: u8) -> SerialStatus;
```

**Port Numbers:**
- 0 = COM1 (I/O base 0x03F8)
- 1 = COM2 (I/O base 0x02F8)
- 2 = COM3 (I/O base 0x03E8)
- 3 = COM4 (I/O base 0x02E8)

**Example Usage in Assembly:**
```asm
; Initialize COM1 at 9600 baud, no parity, 1 stop bit, 8 bits
mov dx, 0          ; DX = port number (0 = COM1)
mov al, 0xE3       ; AL = parameters (111=9600, 00=no parity, 0=1 stop, 11=8 bits)
mov ah, 0x00       ; AH = function 00h (initialize)
int 0x14           ; Call serial services
; AH = line status, AL = modem status

; Write a character to COM1
mov dx, 0          ; DX = port number
mov al, 'A'        ; AL = character to send
mov ah, 0x01       ; AH = function 01h (write character)
int 0x14           ; Call serial services
; AH = line status (bit 7 set if timeout)

; Read a character from COM1
mov dx, 0          ; DX = port number
mov ah, 0x02       ; AH = function 02h (read character)
int 0x14           ; Call serial services
; AH = line status, AL = received character (if no timeout)

; Get serial port status
mov dx, 0          ; DX = port number
mov ah, 0x03       ; AH = function 03h (get status)
int 0x14           ; Call serial services
; AH = line status, AL = modem status
```

**Note:** The default native and WASM implementations return timeout errors for all serial port operations, as actual serial port hardware is not available. Platform-specific implementations can provide real serial port access if needed.

**Printer Services (INT 17h):**

The emulator implements BIOS printer services through the `Bios` trait. Printer operations allow sending output to parallel printer ports (LPT1-LPT3).

**Printer Status Bits (returned in AH):**
- Bit 0: Timeout
- Bit 3: I/O error
- Bit 4: Printer selected
- Bit 5: Out of paper
- Bit 6: Acknowledge
- Bit 7: Not busy (0 = busy, 1 = ready)

**Implementing Printer Operations in Platform Code:**

To support printers, implement these `Bios` trait methods:

```rust
fn printer_init(&mut self, printer: u8) -> PrinterStatus;
fn printer_write(&mut self, printer: u8, ch: u8) -> PrinterStatus;
fn printer_status(&self, printer: u8) -> PrinterStatus;
```

**Printer Numbers:**
- 0 = LPT1 (I/O base 0x0378)
- 1 = LPT2 (I/O base 0x0278)
- 2 = LPT3 (I/O base 0x03BC)

**Example Usage in Assembly:**
```asm
; Initialize LPT1
mov dx, 0          ; DX = printer number (0 = LPT1)
mov ah, 0x01       ; AH = function 01h (initialize)
int 0x17           ; Call printer services
; AH = printer status

; Print a character to LPT1
mov dx, 0          ; DX = printer number
mov al, 'A'        ; AL = character to print
mov ah, 0x00       ; AH = function 00h (print character)
int 0x17           ; Call printer services
; AH = printer status

; Get printer status
mov dx, 0          ; DX = printer number
mov ah, 0x02       ; AH = function 02h (get status)
int 0x17           ; Call printer services
; AH = printer status
```

**Note:** The default native and WASM implementations return timeout errors for all printer operations, as actual printer hardware is not available. Platform-specific implementations can provide real printer access if needed.

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

- **AH=0Eh - Select Default Disk:**
  - Input: DL = drive number (0=A, 1=B, etc.)
  - Output: AL = number of logical drives in system
  - Sets the current default drive
  - In the native implementation, always returns 1 (only drive A available)

- **AH=37h - Get/Set Switch Character:**
  - Input: AL = 0 (get) or 1 (set), DL = new switch character (when AL=1)
  - Output: DL = switch character (when AL=0), AL = 0xFF (function not supported)
  - This function is obsolete in DOS 5.0+ and returns AL=0xFF to indicate not supported
  - For compatibility, returns '/' as the switch character when queried

- **AH=44h - IOCTL (Input/Output Control):**
  - Input: AL = subfunction, BX = file handle
  - Provides device-specific control operations
  - **Implemented subfunctions:**
    - AL=00h: Get device information - Returns DX = device info word
    - AL=01h: Set device information - Sets device info from DX
    - AL=06h: Get input status - Returns AL=0xFF if ready
    - AL=07h: Get output status - Returns AL=0xFF if ready
    - AL=08h: Check if block device is removable - Returns AL=1 (fixed)
    - AL=09h: Check if block device is remote - Returns DX bit 12 clear (local)
    - AL=0Ah: Check if handle is remote - Returns DX bit 15 clear (local)
  - **Device information word (for AL=00h/01h):**
    - Bit 7: 1 = character device, 0 = disk file
    - Bit 6: 0 = EOF on input (files only)
    - Bit 5: 0 = binary mode, 1 = cooked mode
    - Bit 0: 1 = console input device
    - Bit 1: 1 = console output device

- **AH=48h - Allocate Memory:**
  - Input: BX = number of paragraphs (16-byte blocks) to allocate
  - Output (success): CF clear, AX = segment of allocated memory block
  - Output (failure): CF set, AX = error code, BX = size of largest available block
  - **Implementation:** Native platform implements a simple memory allocator
    - Allocates from segment 0x2000 to 0xA000 (conventional memory)
    - Uses a bump allocator strategy with HashMap tracking
    - Supports approximately 512KB of allocatable memory

- **AH=49h - Free Memory:**
  - Input: ES = segment of memory block to free
  - Output (success): CF clear
  - Output (failure): CF set, AX = error code
  - **Implementation:** Removes block from allocation table
    - Validates segment address belongs to an allocated block
    - Returns INVALID_MEMORY_BLOCK_ADDRESS error if segment not found

- **AH=4Ah - Resize Memory Block:**
  - Input: ES = segment of block to resize, BX = new size in paragraphs
  - Output (success): CF clear
  - Output (failure): CF set, AX = error code, BX = maximum size available
  - **Implementation:** Resizes existing memory blocks
    - Shrinking always succeeds (reduces block size)
    - Growing only succeeds if block is the last allocated block
    - Returns INSUFFICIENT_MEMORY if cannot resize in place

- **AH=50h - Set PSP Address:**
  - Input: BX = segment of new Program Segment Prefix
  - Sets the current PSP segment for the running program
  - PSP tracking is not fully implemented in the simple BIOS but the function is available for compatibility

**Interrupt Vector Table (IVT):**

The IVT is located at memory address 0000:0000 and contains 256 entries (one for each possible interrupt):
- Each entry is 4 bytes: 2-byte offset followed by 2-byte segment
- Entry for interrupt N is at address N * 4
- The IVT occupies the first 1KB of memory (0x0000-0x03FF)
- Programs can read and modify interrupt vectors using INT 21h functions 25h and 35h

**Critical Files:**
- [core/src/cpu/bios/mod.rs](core/src/cpu/bios/mod.rs) - BIOS trait definition and interrupt dispatch
- [core/src/cpu/bios/int10.rs](core/src/cpu/bios/int10.rs) - Video services (INT 10h)
- [core/src/cpu/bios/int13.rs](core/src/cpu/bios/int13.rs) - Disk services (INT 13h)
- [core/src/cpu/bios/int16.rs](core/src/cpu/bios/int16.rs) - Keyboard services (INT 16h)
- [core/src/cpu/bios/int21.rs](core/src/cpu/bios/int21.rs) - DOS services (INT 21h)
- [core/src/cpu/mod.rs](core/src/cpu/mod.rs) - CPU and interrupt dispatch (`execute_int_with_io`)
- [core/src/lib.rs](core/src/lib.rs) - Computer integration (INT opcode detection)
- [core/src/cpu/instructions/control_flow.rs](core/src/cpu/instructions/control_flow.rs) - INT instruction implementation

### Boot Process

The emulator supports booting from disk images, simulating the BIOS boot process for the Intel 8086. This allows running bootable disk images including operating systems like MS-DOS.

**Boot Sequence:**

1. **Load Boot Sector**: Read sector 0 (cylinder 0, head 0, sector 1) from disk using INT 13h services
2. **Verify Boot Signature**: Check for 0x55AA signature at bytes 510-511
3. **Load to Memory**: Copy 512-byte boot sector to physical address 0x7C00 (segment 0x0000, offset 0x7C00)
4. **Initialize Registers**: Set up CPU registers as BIOS would:
   - CS:IP = 0x0000:0x7C00 (boot sector entry point)
   - DL = boot drive number (0x00 for floppy A:, 0x80 for first hard disk)
   - SS:SP = 0x0000:0x7C00 (stack just below boot sector)
   - DS = ES = 0x0000
5. **Transfer Control**: CPU begins executing boot sector code

**Implementation** ([core/src/lib.rs](core/src/lib.rs)):

The `Computer::boot()` method handles the boot process:

```rust
pub fn boot(&mut self, drive: u8) -> Result<()>
```

**Parameters:**
- `drive`: Boot drive number (0x00 for floppy A:, 0x80 for first hard disk)

**Returns:**
- `Ok(())` if boot sector loaded successfully
- `Err` if boot sector read fails, is not 512 bytes, or has invalid signature

**Usage with Native CLI:**

Boot from a disk image:
```bash
cargo run -p emu86-native -- --boot --disk <disk_image.img>
```

Options:
- `--boot`: Enable boot mode (required)
- `--disk <path>`: Path to disk image file (optional, creates empty 1.44MB floppy if not specified)
- `--boot-drive <0xNN>`: Boot drive number (default: 0x00 for floppy)

**Creating a Boot Disk:**

1. Write a boot sector in assembly ([examples/boot_test.asm](examples/boot_test.asm)):
   ```nasm
   [BITS 16]
   [ORG 0x7C00]

   start:
       ; Your boot sector code here
       ; ...
       hlt

   ; Boot signature (must be at offset 510-511)
   times 510-($-$$) db 0
   dw 0xAA55  ; Little-endian: 0x55 0xAA
   ```

2. Assemble the boot sector:
   ```bash
   nasm -f bin boot_test.asm -o boot_test.bin
   ```

3. Create disk image and copy boot sector:
   ```bash
   # Create 1.44MB floppy disk image
   dd if=/dev/zero of=boot_disk.img bs=512 count=2880

   # Copy boot sector to first sector
   dd if=boot_test.bin of=boot_disk.img bs=512 count=1 conv=notrunc
   ```

4. Boot from the disk:
   ```bash
   cargo run -p emu86-native -- --boot --disk boot_disk.img
   ```

**Boot Sector Requirements:**

- Exactly 512 bytes in size
- Boot signature 0x55AA at bytes 510-511 (little-endian)
- Code starts at offset 0 (will be loaded at 0x7C00)
- Can use BIOS interrupts (INT 10h, INT 13h, INT 16h, etc.)
- DL register contains boot drive number on entry

**Example:**

See [examples/boot_test.asm](examples/boot_test.asm) for a complete working boot sector example.

**MS-DOS Boot:**

To boot MS-DOS or other operating systems:
1. Create a disk image with MS-DOS boot sector
2. Ensure the boot sector loads the DOS kernel (IO.SYS, MSDOS.SYS)
3. Boot using `--boot --disk dos_disk.img`

The boot sector is responsible for loading the operating system from disk into memory and transferring control to it.

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
