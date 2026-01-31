# Implementation Plan: Microsoft Serial Mouse Support

## Overview

Implement Microsoft Serial Mouse protocol support for COM1/COM2, allowing DOS mouse drivers (MOUSE.COM) to detect and use the mouse via serial port. This works alongside the existing INT 33h implementation - the serial mouse sends data packets over the serial port, DOS drivers read this data and provide INT 33h services.

## Architecture

### Component Design

```
Platform Events (crossterm/winit)
    ↓
MouseInput trait (TerminalMouse/GuiMouse)
    ↓
SerialMouse (implements SerialDevice trait)
    ↓
SerialPortController [FIFO buffering + UART registers + attached device]
    ↓
INT 14h / I/O Ports 0x3F8-0x3FF
    ↓
DOS Mouse Driver (MOUSE.COM)
    ↓
INT 33h services
```

### Key Design Decisions

1. **Serial mouse works alongside existing INT 33h** - Serial mouse sends data to DOS driver which provides INT 33h. More realistic, matches real hardware.

2. **Both INT 14h and I/O port emulation** - Implement proper UART registers at 0x3F8-0x3FF (COM1) and 0x2F8-0x2FF (COM2) for hardware detection, plus INT 14h for data access.

3. **Full UART emulation** - Implement key 16450 UART registers for realism:
   - Data Register (0x3F8)
   - Interrupt Enable Register (0x3F9)
   - Line Control Register (0x3FB) with DLAB support
   - Line Status Register (0x3FD) - critical for data ready flag
   - Modem Status Register (0x3FE)

4. **SerialDevice trait** - Clean abstraction allowing different devices (mouse, modem, null modem, etc.) to be attached to serial ports. Each SerialPortController can have an optional attached device.

5. **Computer owns device lifecycle** - `Computer` provides `set_com1_device()` and `set_com2_device()` methods to attach devices, keeping the API clean and the architecture modular.

### Microsoft Serial Mouse Protocol

3-byte packets sent at 1200 baud, 7N1:

```
Byte 1: 0x40 | (LB<<5) | (RB<<4) | (Y7<<3) | (Y6<<2) | (X7<<1) | X6
Byte 2: X delta (6-bit signed, -32 to +31)
Byte 3: Y delta (6-bit signed, -32 to +31)

Where:
- LB = left button (1=pressed)
- RB = right button (1=pressed)
- X7,X6 = high 2 bits of X delta (sign extension)
- Y7,Y6 = high 2 bits of Y delta (sign extension)
```

## Implementation Steps

### ✅ Phases 1-6: Core Infrastructure and CLI Configuration (COMPLETED)

All core serial port infrastructure and CLI configuration have been implemented and tested:
- Phase 1: Serial Port Infrastructure - ✅ COMPLETED
- Phase 2: SerialDevice Trait and SerialMouse - ✅ COMPLETED
- Phase 3: BIOS Integration - ✅ COMPLETED
- Phase 4: I/O Port Emulation - ✅ COMPLETED
- Phase 5: Computer Integration - ✅ COMPLETED
- Phase 6: CLI Configuration - ✅ COMPLETED

The following components are now functional:
- `core/src/serial_port.rs` - UART register emulation (16450 compatible)
- `core/src/serial_mouse.rs` - SerialMouse implementing SerialDevice trait
- BIOS serial port controllers and INT 14h integration
- I/O port routing (0x3F8-0x3FF for COM1, 0x2F8-0x2FF for COM2)
- Computer device management methods (set_com1_device, set_com2_device, etc.)
- Periodic serial device updates (every 1000 instructions)
- CLI arguments `--com1` and `--com2` for attaching serial devices
- `native/src/terminal_mouse.rs` - Shared mouse state via Arc<Mutex<...>> for serial mouse

### ~~Phase 6: CLI Configuration~~ ✅ COMPLETED

CLI arguments `--com1` and `--com2` have been added to `native/src/main.rs` to allow attaching serial devices at startup.

**Example usage:**
```bash
cargo run -p emu86-native -- --boot --floppy-a dos.img --com1 mouse
cargo run -p emu86-native -- --boot --floppy-a dos.img --com1 mouse --com2 mouse
```

**Implementation notes:**
- `TerminalMouse` now uses `Arc<Mutex<SharedMouseState>>` internally to allow state sharing
- `clone_shared()` method creates new instances sharing the same mouse state
- One instance is passed to Computer (for BIOS/INT 33h), another can be used for SerialMouse

### Phase 7: Testing

**Create test program: `examples/mouse_serial.asm`**

```nasm
; Test serial mouse by reading from COM1
org 0x100

section .text
start:
    ; Initialize COM1 to 1200 baud, 7N1
    mov ah, 0x00
    mov al, 10000010b  ; 1200 baud, no parity, 1 stop, 7 bits
    mov dx, 0          ; COM1
    int 0x14

    ; Print "Waiting for mouse..."
    mov dx, msg_wait
    mov ah, 0x09
    int 0x21

.loop:
    ; Check if data ready
    mov ah, 0x03       ; Get status
    mov dx, 0          ; COM1
    int 0x14
    test al, 0x01      ; Data ready bit
    jz .loop

    ; Read byte
    mov ah, 0x02       ; Read char
    mov dx, 0
    int 0x14

    ; Display byte in hex
    mov bl, al
    call print_hex

    jmp .loop

print_hex:
    ; Print BL as hex
    push bx
    mov al, bl
    shr al, 4
    call print_nibble
    pop bx
    mov al, bl
    and al, 0x0F
    call print_nibble
    mov dl, ' '
    mov ah, 0x02
    int 0x21
    ret

print_nibble:
    add al, '0'
    cmp al, '9'
    jle .digit
    add al, 7
.digit:
    mov dl, al
    mov ah, 0x02
    int 0x21
    ret

section .data
msg_wait: db 'Waiting for serial mouse data on COM1...', 13, 10, '$'
```

Compile and run:
```bash
nasm -f bin examples/mouse_serial.asm -o mouse_serial.com
cargo run -p emu86-native -- --boot --floppy-a dos.img --com1 mouse
# Copy mouse_serial.com to DOS disk, run it, move mouse
```

## Critical Files

| File | Purpose |
|------|---------|
| `core/src/serial_port.rs` | ✅ SerialPortController with UART emulation, SerialDevice trait |
| `core/src/serial_mouse.rs` | NEW - SerialMouse implementing SerialDevice trait |
| `core/src/cpu/bios/mod.rs` | Add serial_ports array, update_serial_devices(), I/O methods |
| `core/src/cpu/instructions/io.rs` | Route serial I/O ports (0x3F8+) to Bios |
| `core/src/computer.rs` | Add set_com1/2_device() methods, call update_serial_devices() |
| `native/src/main.rs` | Add --com1/--com2 CLI args, attach devices on startup |
| `core/src/lib.rs` | Export serial_port and serial_mouse modules |

## Verification

1. **Compile all crates**:
   ```bash
   cargo build
   cargo clippy
   ```

2. **Run with serial mouse enabled**:
   ```bash
   cargo run -p emu86-native -- --boot --floppy-a dos.img --com1 mouse
   ```

3. **Test with DOS mouse driver**:
   - Copy MOUSE.COM to DOS disk
   - Run: `MOUSE /S1` (serial mouse on COM1)
   - MOUSE.COM should detect the mouse
   - Test INT 33h with DOS programs

4. **Manual packet verification**:
   - Use test program to read raw bytes from COM1
   - Move mouse, verify 3-byte packets: 0x4X, 0xXX, 0xXX
   - Click buttons, verify bit 5 (left) and bit 4 (right) in byte 1

5. **End-to-end test**:
   - Run Norton Commander or DOS Shell
   - Verify mouse cursor moves
   - Verify clicks work
   - Verify smooth motion tracking

## Expected Behavior

- DOS mouse driver detects Microsoft Serial Mouse on COM1/COM2
- Mouse movements generate 3-byte packets in serial buffer
- INT 14h reads return mouse data
- DOS driver provides INT 33h services
- Existing programs using INT 33h work with serial mouse input
- I/O port reads (0x3F8+) return proper UART register values
