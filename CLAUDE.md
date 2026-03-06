# Oxide86 - x86 Emulator

Intel x86 CPU emulator in Rust with native and WebAssembly support.

## Coding Rules

- Code in core must support both native and wasm
- Don't worry about backwards compatibility
- Run ./scripts/pre-commit.sh when done
- Instead of running cargo build or clippy run ./scripts/pre-commit.sh instead
- logs are written to oxide86.log
- when logging unimplemented features use log::warn!

## Testing

- Assembly files to test various aspects of the emulator are found in core/src/test_data
- The assembly files (.asm) are compiled in core/build.rs using nasm
- Tests are run from core/src/tests.rs
- You can run the tests using `cargo test --all` you don't need to run pre-commit.sh

## Bus, IO, and Memory Architecture

### Device trait (`core/src/lib.rs`)
All hardware devices implement the `Device` trait:
- `io_read_u8(port: u16) -> Option<u8>` — return `Some(val)` to claim the port, `None` to pass
- `io_write_u8(port: u16, val: u8) -> bool` — return `true` to claim the port, `false` to pass
- `memory_read_u8(addr: usize) -> Option<u8>` — same pattern for memory-mapped IO
- `memory_write_u8(addr: usize, val: u8) -> bool` — same pattern
- `reset(&mut self)` — called on system reset
- `as_any(&self) -> &dyn Any`

`DeviceRef = Rc<RefCell<dyn Device>>`

### Bus (`core/src/bus.rs`)
The `Bus` owns `Memory` and a `Vec<DeviceRef>`. It routes:
- **IO ports**: `io_read_u8` / `io_write_u8` iterate all devices; first to return `Some`/`true` wins. Unhandled reads return `0xFF`.
- **Memory**: `memory_read_u8` / `memory_write_u8` check devices first **only** for the memory-mapped IO range `0xA0000..0xF0000`. Outside that range, accesses go directly to `Memory`.
- u16/u32 memory ops are built on top of u8 ops (little-endian).

### Memory (`core/src/memory.rs`)
Plain `Vec<u8>`. Reads beyond size return `0xFF`; writes beyond size are silently ignored.

### Adding a new device
1. Implement the `Device` trait.
2. Either add it to `Bus::new` (for core devices with named fields) or call `bus.add_device(device)`.
3. Handle the relevant IO ports or memory range in `io_read_u8` / `io_write_u8` / `memory_read_u8` / `memory_write_u8`.

## Resources
- [8086 User Manual](https://edge.edx.org/c4x/BITSPilani/EEE231/asset/8086_family_Users_Manual_1_.pdf)
- [x86 Reference](https://www.felixcloutier.com/x86/)
- [8086 Opcodes](http://www.mlsite.net/8086/)
