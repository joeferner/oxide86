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

### ✅ Phase 1: Serial Port Infrastructure (COMPLETED)

Implemented `core/src/serial_port.rs` with `SerialPortController` struct including:
- UART register emulation (16450 compatible)
- RX/TX buffers with FIFO support
- DLAB support for baud rate divisor configuration
- Line status and modem status registers
- Full test coverage

Updated `core/src/lib.rs` to export the serial_port module.


### ✅ Phase 2: SerialDevice Trait and SerialMouse (COMPLETED)

Implemented the SerialDevice trait and SerialMouse:
- Added SerialDevice trait to `core/src/serial_port.rs`
- Updated SerialPortController to support attached devices (attach_device, detach_device, update_device, on_init, on_write)
- Created `core/src/serial_mouse.rs` with SerialMouse implementation
- SerialMouse generates Microsoft Serial Mouse protocol packets (3-byte packets)
- Full test coverage for packet generation and initialization
- Updated `core/src/lib.rs` to export serial_mouse module

### ✅ Phase 3: BIOS and core/src/mouse.rs Integration (COMPLETED)

Implemented BIOS integration for serial port controllers:
- Added `serial_ports: [SerialPortController; 2]` to Bios struct
- Added I/O port access methods (`serial_io_read`, `serial_io_write`)
- Added `update_serial_devices()` method for periodic device updates
- Updated INT 14h methods to use SerialPortController instead of peripheral stubs
- Consolidated duplicate `SerialParams` and `SerialStatus` types (now defined in serial_port.rs)
- Added `set_com1_device`, `set_com2_device`, `clear_com1_device`, `clear_com2_device` to Computer
- Added serial device update calls in Computer::step (every 1000 instructions)

### Phase 2 Original Plan (for reference)

**File: `core/src/serial_port.rs`**

Add trait for serial devices:

```rust
/// Trait for devices that can be attached to serial ports
pub trait SerialDevice {
    /// Called when the port is initialized with new parameters
    /// Returns optional initialization response bytes (e.g., "M" for Microsoft Mouse)
    fn on_init(&mut self, params: &SerialParams) -> Option<Vec<u8>>;

    /// Called periodically to allow device to generate data
    /// Returns bytes to enqueue into the RX buffer
    fn update(&mut self) -> Vec<u8>;

    /// Called when a byte is written to the serial port
    /// Allows device to respond to commands
    fn on_write(&mut self, byte: u8);
}
```

Update SerialPortController to hold an optional device:

```rust
pub struct SerialPortController {
    pub params: SerialParams,
    pub rx_buffer: VecDeque<u8>,
    pub line_control: u8,
    pub modem_status: u8,
    pub divisor_latch: u16,
    pub device: Option<Box<dyn SerialDevice>>,
}

impl SerialPortController {
    pub fn new() -> Self {
        Self {
            params: SerialParams::default(),
            rx_buffer: VecDeque::new(),
            line_control: 0,
            modem_status: modem_status::DATA_SET_READY | modem_status::CLEAR_TO_SEND,
            divisor_latch: 96,
            device: None,
        }
    }

    /// Attach a device to this serial port
    pub fn attach_device(&mut self, device: Box<dyn SerialDevice>) {
        self.device = Some(device);
    }

    /// Detach the current device
    pub fn detach_device(&mut self) {
        self.device = None;
        self.rx_buffer.clear();
    }

    /// Update attached device and queue any generated bytes
    pub fn update_device(&mut self) {
        if let Some(ref mut device) = self.device {
            let bytes = device.update();
            for byte in bytes {
                self.enqueue_byte(byte);
            }
        }
    }

    /// Call when port is initialized
    pub fn on_init(&mut self, params: SerialParams) {
        self.params = params;
        if let Some(ref mut device) = self.device {
            if let Some(response) = device.on_init(&params) {
                for byte in response {
                    self.enqueue_byte(byte);
                }
            }
        }
    }

    /// Call when byte is written to port
    pub fn on_write(&mut self, byte: u8) {
        if let Some(ref mut device) = self.device {
            device.on_write(byte);
        }
    }

    // ... existing methods ...
}
```

**File: `core/src/serial_mouse.rs` (NEW)**

Create SerialMouse implementation:

```rust
use crate::serial_port::{SerialDevice, SerialParams};
use crate::MouseInput;

pub struct SerialMouse {
    mouse_input: Box<dyn MouseInput>,
    last_buttons: u8,
    accumulated_x: i16,
    accumulated_y: i16,
    motion_threshold: u16,
}

impl SerialMouse {
    pub fn new(mouse_input: Box<dyn MouseInput>) -> Self {
        Self {
            mouse_input,
            last_buttons: 0,
            accumulated_x: 0,
            accumulated_y: 0,
            motion_threshold: 8,
        }
    }
}

impl SerialDevice for SerialMouse {
    fn on_init(&mut self, params: &SerialParams) -> Option<Vec<u8>> {
        // Check if initialized to Microsoft Mouse settings (1200 baud, 7N1)
        if params.baud_rate == 0x04 && params.word_length == 0x02 {
            Some(vec![b'M'])  // Send identification byte
        } else {
            None
        }
    }

    fn update(&mut self) -> Vec<u8> {
        let state = self.mouse_input.get_state();
        let (dx, dy) = self.mouse_input.get_motion();

        self.accumulated_x += dx;
        self.accumulated_y += dy;

        let current_buttons = encode_buttons(&state);
        let buttons_changed = current_buttons != self.last_buttons;
        let motion_exceeded = self.accumulated_x.abs() >= self.motion_threshold as i16
                           || self.accumulated_y.abs() >= self.motion_threshold as i16;

        if buttons_changed || motion_exceeded {
            let packet = generate_ms_mouse_packet(
                state.left_button,
                state.right_button,
                self.accumulated_x,
                self.accumulated_y,
            );

            self.accumulated_x = 0;
            self.accumulated_y = 0;
            self.last_buttons = current_buttons;

            packet.to_vec()
        } else {
            Vec::new()
        }
    }

    fn on_write(&mut self, _byte: u8) {
        // Microsoft Serial Mouse doesn't respond to commands
    }
}

fn encode_buttons(state: &crate::MouseState) -> u8 {
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

### Phase 3: BIOS Integration (COMPLETED - See above)

**File: `core/src/cpu/bios/mod.rs`**

Add to Bios struct:

```rust
pub struct Bios<K: KeyboardInput> {
    pub shared: SharedBiosState,
    pub keyboard: K,
    pub mouse: Box<dyn MouseInput>,

    // Serial port controllers
    pub serial_ports: [SerialPortController; 2],
}
```

Add methods for serial port I/O and updates:

```rust
impl<K: KeyboardInput> Bios<K> {
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

    /// Update all serial port devices
    pub fn update_serial_devices(&mut self) {
        for port in &mut self.serial_ports {
            port.update_device();
        }
    }
}
```

Update INT 14h serial methods to use controllers:

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

    let controller = &mut self.serial_ports[port as usize];
    controller.on_write(ch);

    line_status::TRANSMIT_HOLDING_EMPTY | line_status::TRANSMIT_SHIFT_EMPTY
}

pub fn serial_init(&mut self, port: u8, params: SerialParams) -> SerialStatus {
    if port > 1 {
        return SerialStatus {
            line_status: line_status::TIMEOUT,
            modem_status: 0,
        };
    }

    self.serial_ports[port as usize].on_init(params);

    SerialStatus {
        line_status: line_status::TRANSMIT_HOLDING_EMPTY | line_status::TRANSMIT_SHIFT_EMPTY,
        modem_status: modem_status::DATA_SET_READY | modem_status::CLEAR_TO_SEND,
    }
}
```

### Phase 4: I/O Port Emulation

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

### Phase 5: Computer Integration

**File: `core/src/lib.rs`**

Export new modules:

```rust
pub mod serial_port;
pub mod serial_mouse;
```

**File: `core/src/computer.rs`**

Add methods to attach serial devices:

```rust
use crate::serial_mouse::SerialMouse;
use crate::serial_port::SerialDevice;

impl Computer {
    /// Attach a device to COM1
    pub fn set_com1_device(&mut self, device: Box<dyn SerialDevice>) {
        self.bios.serial_ports[0].attach_device(device);
    }

    /// Attach a device to COM2
    pub fn set_com2_device(&mut self, device: Box<dyn SerialDevice>) {
        self.bios.serial_ports[1].attach_device(device);
    }

    /// Remove device from COM1
    pub fn clear_com1_device(&mut self) {
        self.bios.serial_ports[0].detach_device();
    }

    /// Remove device from COM2
    pub fn clear_com2_device(&mut self) {
        self.bios.serial_ports[1].detach_device();
    }
}
```

Add periodic serial device update in execution loop. Find the `run()` or main execution method:

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

        // Update serial devices after each batch
        self.bios.update_serial_devices();
    }
}
```

Or if step-by-step execution, call every N steps:

```rust
pub fn step(&mut self) {
    self.cpu.execute_instruction(&mut self.memory, &mut self.bios, &mut self.io_device);
    self.increment_cycles();

    // Update serial devices every 1000 instructions (~18 times per second)
    if self.cycle_count % 1000 == 0 {
        self.bios.update_serial_devices();
    }
}
```

### Phase 6: CLI Configuration

**File: `native/src/main.rs`**

Add CLI arguments:

```rust
#[derive(Parser)]
struct Cli {
    // ... existing fields ...

    /// Device to attach to COM1 (e.g., "mouse", "null")
    #[arg(long = "com1", value_name = "DEVICE")]
    com1_device: Option<String>,

    /// Device to attach to COM2 (e.g., "mouse", "null")
    #[arg(long = "com2", value_name = "DEVICE")]
    com2_device: Option<String>,
}
```

Initialize serial devices after creating Computer:

```rust
use emu86_core::serial_mouse::SerialMouse;

// After computer creation
if let Some(device) = cli.com1_device {
    match device.as_str() {
        "mouse" => {
            let mouse_input = computer.bios.mouse.clone_input();
            computer.set_com1_device(Box::new(SerialMouse::new(mouse_input)));
            eprintln!("Serial mouse attached to COM1");
        }
        _ => {
            eprintln!("Warning: Unknown device '{}' for COM1", device);
        }
    }
}

if let Some(device) = cli.com2_device {
    match device.as_str() {
        "mouse" => {
            let mouse_input = computer.bios.mouse.clone_input();
            computer.set_com2_device(Box::new(SerialMouse::new(mouse_input)));
            eprintln!("Serial mouse attached to COM2");
        }
        _ => {
            eprintln!("Warning: Unknown device '{}' for COM2", device);
        }
    }
}
```

**Example usage:**
```bash
cargo run -p emu86-native -- --boot --floppy-a dos.img --com1 mouse
cargo run -p emu86-native -- --boot --floppy-a dos.img --com1 mouse --com2 joystick
```

**Note:** The MouseInput trait will need a `clone_input()` or similar method to share mouse state between the serial device and the existing mouse handler. Alternative: pass mouse reference to SerialMouse and make it generic over lifetime, or use Arc/Rc.

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
