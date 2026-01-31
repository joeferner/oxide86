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
Bios::update_serial_mouse() [generates MS Mouse packets]
    ↓
SerialPortController [FIFO buffering + UART registers]
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

4. **SerialMouseState in Bios** - Avoids trait object downcasting, allows direct access to serial ports for packet injection.

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

### Phase 1: Serial Port Infrastructure

**File: `core/src/serial_port.rs` (NEW)**

Create `SerialPortController` struct:

```rust
pub struct SerialPortController {
    port_number: u8,              // 0=COM1, 1=COM2
    base_port: u16,               // 0x3F8 or 0x2F8

    // Buffers
    rx_buffer: VecDeque<u8>,      // Receive FIFO (256 bytes)
    tx_buffer: VecDeque<u8>,      // Transmit FIFO

    // UART Registers (16450 compatible)
    interrupt_enable: u8,         // IER
    line_control: u8,             // LCR (includes DLAB bit 7)
    modem_control: u8,            // MCR
    line_status: u8,              // LSR (bit 0=data ready)
    modem_status: u8,             // MSR
    scratch: u8,                  // Scratch register
    divisor_latch: u16,           // Baud rate divisor (when DLAB=1)

    // Configuration
    params: SerialParams,
    buffer_size: usize,
}
```

Key methods:
- `new(port: u8) -> Self` - Initialize COM1 (0) or COM2 (1)
- `enqueue_byte(&mut self, byte: u8) -> bool` - Add to RX buffer, update LSR
- `dequeue_byte(&mut self) -> Option<u8>` - Read from RX, update LSR
- `read_register(&self, offset: u16) -> u8` - Read UART register (0-7)
- `write_register(&mut self, offset: u16, value: u8)` - Write UART register
- `get_line_status(&self) -> u8` - Returns LSR with DATA_READY bit
- `reset(&mut self)` - Clear buffers, reset registers

**File: `core/src/lib.rs`**

Add: `pub mod serial_port;`

### Phase 2: BIOS Integration

**File: `core/src/cpu/bios/mod.rs`**

Add to Bios struct:

```rust
pub struct Bios<K: KeyboardInput> {
    pub shared: SharedBiosState,
    pub keyboard: K,
    pub mouse: Box<dyn MouseInput>,

    // Serial port controllers
    pub serial_ports: [SerialPortController; 2],

    // Serial mouse configuration (if enabled)
    pub serial_mouse_config: Option<SerialMouseState>,
}

pub struct SerialMouseState {
    pub target_port: u8,          // 0=COM1, 1=COM2
    pub last_buttons: u8,         // Last sent button state
    pub accumulated_x: i16,       // Mickeys since last packet
    pub accumulated_y: i16,       // Mickeys since last packet
    pub motion_threshold: u16,    // Send packet after this many mickeys (default: 8)
}
```

Update `Bios::new()`:

```rust
pub fn new(keyboard: K, mouse: Box<dyn MouseInput>) -> Self {
    Self {
        shared: SharedBiosState::new(),
        keyboard,
        mouse,
        serial_ports: [
            SerialPortController::new(0),  // COM1
            SerialPortController::new(1),  // COM2
        ],
        serial_mouse_config: None,  // Disabled by default
    }
}
```

Add methods:

```rust
// Enable serial mouse on specified COM port (0 or 1)
pub fn enable_serial_mouse(&mut self, port: u8) {
    self.serial_mouse_config = Some(SerialMouseState {
        target_port: port,
        last_buttons: 0,
        accumulated_x: 0,
        accumulated_y: 0,
        motion_threshold: 8,
    });
}

// Update serial mouse - generate packets if needed
pub fn update_serial_mouse(&mut self) {
    if let Some(ref mut config) = self.serial_mouse_config {
        let state = self.mouse.get_state();
        let (dx, dy) = self.mouse.get_motion();

        config.accumulated_x += dx;
        config.accumulated_y += dy;

        let current_buttons = encode_buttons(&state);
        let buttons_changed = current_buttons != config.last_buttons;
        let motion_exceeded = config.accumulated_x.abs() >= config.motion_threshold as i16
                           || config.accumulated_y.abs() >= config.motion_threshold as i16;

        if buttons_changed || motion_exceeded {
            let packet = generate_ms_mouse_packet(
                state.left_button,
                state.right_button,
                config.accumulated_x,
                config.accumulated_y,
            );

            // Enqueue to serial port
            for byte in packet.iter() {
                self.serial_ports[config.target_port as usize].enqueue_byte(*byte);
            }

            config.accumulated_x = 0;
            config.accumulated_y = 0;
            config.last_buttons = current_buttons;
        }
    }
}

// I/O port access for serial ports
pub fn serial_io_read(&self, port: u8, offset: u16) -> u8 {
    if port > 1 {
        return 0xFF;
    }
    self.serial_ports[port as usize].read_register(offset)
}

pub fn serial_io_write(&mut self, port: u8, offset: u16, value: u8) {
    if port > 1 {
        return;
    }
    self.serial_ports[port as usize].write_register(offset, value);
}
```

Add helper functions in mod.rs:

```rust
fn encode_buttons(state: &MouseState) -> u8 {
    let mut buttons = 0u8;
    if state.left_button { buttons |= 0x01; }
    if state.right_button { buttons |= 0x02; }
    if state.middle_button { buttons |= 0x04; }
    buttons
}

fn generate_ms_mouse_packet(left: bool, right: bool, dx: i16, dy: i16) -> [u8; 3] {
    // Clamp deltas to -32..+31 range (6-bit signed)
    let dx = dx.clamp(-32, 31) as i8;
    let dy = dy.clamp(-32, 31) as i8;

    // Extract high bits for byte 1
    let x_hi = ((dx >> 6) & 0x03) as u8;
    let y_hi = ((dy >> 6) & 0x03) as u8;

    // Byte 1: sync bit + buttons + high bits
    let byte1 = 0x40
        | (if left { 0x20 } else { 0 })
        | (if right { 0x10 } else { 0 })
        | (y_hi << 2)
        | x_hi;

    // Byte 2: X delta (lower 6 bits)
    let byte2 = (dx & 0x3F) as u8;

    // Byte 3: Y delta (lower 6 bits)
    let byte3 = (dy & 0x3F) as u8;

    [byte1, byte2, byte3]
}
```

Update serial methods to use controllers:

```rust
pub fn serial_read(&mut self, port: u8) -> Result<(u8, u8), u8> {
    if port > 1 {
        return Err(line_status::TIMEOUT);
    }

    let controller = &mut self.serial_ports[port as usize];
    if let Some(byte) = controller.dequeue_byte() {
        let status = controller.get_line_status();
        Ok((byte, status))
    } else {
        Err(line_status::TIMEOUT)
    }
}

pub fn serial_status(&self, port: u8) -> SerialStatus {
    if port > 1 {
        return SerialStatus {
            line_status: line_status::TIMEOUT,
            modem_status: 0,
        };
    }

    let controller = &self.serial_ports[port as usize];
    SerialStatus {
        line_status: controller.get_line_status(),
        modem_status: controller.modem_status,
    }
}

pub fn serial_write(&mut self, port: u8, ch: u8) -> u8 {
    if port > 1 {
        return line_status::TIMEOUT;
    }

    // For now, just acknowledge write (no actual output)
    line_status::TRANSMIT_HOLDING_EMPTY | line_status::TRANSMIT_SHIFT_EMPTY
}

pub fn serial_init(&mut self, port: u8, params: SerialParams) -> SerialStatus {
    if port > 1 {
        return SerialStatus {
            line_status: line_status::TIMEOUT,
            modem_status: 0,
        };
    }

    let controller = &mut self.serial_ports[port as usize];
    controller.params = params;

    // If serial mouse is on this port and initialized to 1200 baud, 7N1
    // send "M" identifier (Microsoft Mouse signature)
    if let Some(ref config) = self.serial_mouse_config {
        if config.target_port == port && is_mouse_config(&params) {
            controller.enqueue_byte(b'M');
        }
    }

    SerialStatus {
        line_status: line_status::TRANSMIT_HOLDING_EMPTY | line_status::TRANSMIT_SHIFT_EMPTY,
        modem_status: modem_status::DATA_SET_READY | modem_status::CLEAR_TO_SEND,
    }
}

fn is_mouse_config(params: &SerialParams) -> bool {
    // 1200 baud (100), no parity, 1 stop bit, 7 bits
    params.baud_rate == 0x04  // 1200 baud
        && params.word_length == 0x02  // 7 bits
}
```

### Phase 3: I/O Port Emulation

**File: `core/src/cpu/instructions/io.rs`**

Update IN/OUT handlers to route serial ports to BIOS:

Find the `in_al_imm8`, `in_ax_imm8`, `in_al_dx`, `in_ax_dx` methods and modify to check for serial ports:

```rust
// Example for in_al_dx
pub fn in_al_dx<K: crate::KeyboardInput>(
    &mut self,
    _memory: &Memory,
    io: &mut crate::cpu::bios::Bios<K>,
) {
    let port = self.dx;
    let value = match port {
        // COM1 registers (0x3F8-0x3FF)
        0x3F8..=0x3FF => io.serial_io_read(0, port - 0x3F8),
        // COM2 registers (0x2F8-0x2FF)
        0x2F8..=0x2FF => io.serial_io_read(1, port - 0x2F8),
        // Other ports use existing io_device
        _ => self.io_device.read_byte(port),
    };
    self.ax = (self.ax & 0xFF00) | (value as u16);
}
```

Similar updates for `out_dx_al`, `out_dx_ax`, `out_imm8_al`, `out_imm8_ax`.

### Phase 4: Computer Integration

**File: `core/src/computer.rs`**

Add periodic serial mouse update in execution loop. Find the `run()` or main execution method:

```rust
pub fn run(&mut self) -> Result<(), String> {
    loop {
        // Execute batch of instructions
        for _ in 0..1000 {
            if self.cpu.halted {
                return Ok(());
            }
            self.step();
        }

        // Update serial mouse after each batch
        self.bios.update_serial_mouse();
    }
}
```

Or if step-by-step execution, call every N steps:

```rust
pub fn step(&mut self) {
    self.cpu.execute_instruction(&mut self.memory, &mut self.bios, &mut self.io_device);
    self.increment_cycles();

    // Update serial mouse every 1000 instructions (~18 times per second at 18.2 KHz)
    if self.cycle_count % 1000 == 0 {
        self.bios.update_serial_mouse();
    }
}
```

### Phase 5: CLI Configuration

**File: `native/src/main.rs`**

Add CLI argument:

```rust
#[derive(Parser)]
struct Cli {
    // ... existing fields ...

    /// Enable Microsoft Serial Mouse on COM port (1=COM1, 2=COM2)
    #[arg(long = "serial-mouse", value_name = "PORT")]
    serial_mouse_port: Option<u8>,
}
```

Initialize serial mouse after creating Computer:

```rust
// After computer creation
if let Some(port) = cli.serial_mouse_port {
    match port {
        1 => {
            computer.bios.enable_serial_mouse(0);  // COM1
            eprintln!("Serial mouse enabled on COM1");
        }
        2 => {
            computer.bios.enable_serial_mouse(1);  // COM2
            eprintln!("Serial mouse enabled on COM2");
        }
        _ => {
            eprintln!("Warning: Invalid COM port {}. Must be 1 or 2.", port);
        }
    }
}
```

### Phase 6: Testing

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
cargo run -p emu86-native -- --serial-mouse 1 --boot --floppy-a dos.img
# Copy mouse_serial.com to DOS disk, run it, move mouse
```

## Critical Files

| File | Purpose |
|------|---------|
| `core/src/serial_port.rs` | NEW - SerialPortController with UART emulation |
| `core/src/cpu/bios/mod.rs` | Add serial_ports array, SerialMouseState, update methods |
| `core/src/cpu/instructions/io.rs` | Route serial I/O ports (0x3F8+) to Bios |
| `core/src/computer.rs` | Call bios.update_serial_mouse() periodically |
| `native/src/main.rs` | Add --serial-mouse CLI arg, enable on startup |
| `core/src/lib.rs` | Export serial_port module |

## Verification

1. **Compile all crates**:
   ```bash
   cargo build
   cargo clippy
   ```

2. **Run with serial mouse enabled**:
   ```bash
   cargo run -p emu86-native -- --boot --floppy-a dos.img --serial-mouse 1
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
