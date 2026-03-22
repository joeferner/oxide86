# Oxide86 - x86 Emulator

Intel x86 CPU emulator in Rust with native and WebAssembly support.

## Coding Rules

- Code in core must support both native and wasm
- Don't worry about backwards compatibility
- Run ./scripts/pre-commit.sh when done
- Instead of running cargo build or clippy run ./scripts/pre-commit.sh instead
- logs are written to oxide86.log
- when logging unimplemented features use log::warn!
- when adding interrupt handlers or io device handling be sure to update decoder
- `use` statements go at the top of the file, not inside functions (unless truly local to the function with no other option)

## Testing

- Assembly files to test various aspects of the emulator are found in core/src/test_data
- The assembly files (.asm) are compiled in core/build.rs using nasm
- Tests are run from core/src/tests.rs
- You can run the tests using `cargo test --all` you don't need to run pre-commit.sh
- Do NOT modify `assert_screen` in `core/src/tests/mod.rs` to auto-save missing PNG snapshots; the user manages snapshot creation manually

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

## Disassembler (`oxide86-disasm`)

Standalone recursive-descent 286 disassembler for COM and EXE files. Output CS:IP addresses match the emulator's execution logs when `loadSegment` is configured correctly.

### Basic usage

```bash
cargo run -p oxide86-disasm -- <file.exe>
cargo run -p oxide86-disasm -- --config target/myprogram.json target/myprogram.exe > target/myprogram.asm
```

### Config file format

```json
{
  "loadSegment": "0EEC",
  "dataSegment": "0EFC",
  "entryPoints": {
    "160F:0000": "main"
  },
  "comments": {
    "160F:0042": "reads key from keyboard"
  },
  "data": {
    "0EFC:6AA0": { "type": "string", "label": "str_joystick_prompt" }
  }
}
```

All fields are optional.

| Field | Description |
|---|---|
| `loadSegment` | Hex segment at which the EXE was loaded in the emulator. Fixes CS values so they match execution logs. |
| `dataSegment` | Hex segment of the data segment (DS). Used to resolve `data` entries. |
| `entryPoints` | Map of `"SEG:OFF"` → label name. Adds extra disassembly roots and gives them named labels. |
| `comments` | Map of `"SEG:OFF"` → comment string. Appended as `; comment` on the instruction at that address. |
| `data` | Map of `"SEG:OFF"` → `{ "type": "string"\|"bytes", "label": "name" }`. Annotates known data regions with labels and type hints. |

### Finding the load segment

When the emulator's entry CS differs from the disassembler's output:

```
emulator entry:      160F:0000
disassembler entry:  0723:0000
load_segment = 0x160F - 0x0723 = 0x0EEC  →  "loadSegment": "0EEC"
```

The load segment is the paragraph DOS allocated for the program (PSP base). The disassembler defaults to `0x0000` (no relocation).

### Address format

All addresses use `SEG:OFF` with hex values, matching the emulator's execution log format (e.g. `160F:0042`). The `0x` prefix is accepted but not required.

### CLI usage

```bash
cargo run -p oxide86-cli -- --reverse-engineer myprogram.exe
# oxide86.asm written on exit
```

Or toggle live in command mode (F12): type `re` to enable, `re` again to disable and write the file.

## Memory Watchpoints

The emulator supports physical-address write watchpoints for debugging. Every write to a watched address is logged to `oxide86.log`:

```
[WATCH] 0x4064E written: 0x64 by 19CD:03AE
```

### CLI usage

```bash
cargo run -p oxide86-cli -- --watch 0x4064E --watch 0x405DC --floppy-a disk.img
```

`--watch` accepts hex addresses (with or without `0x` prefix) and can be repeated for multiple addresses.

### Debugging workflow

When investigating "who wrote value X to address Y?":
1. Convert the target to a physical address (segment × 16 + offset).
2. Re-run the emulator with `--watch 0x<physaddr>`.
3. grep for `[WATCH]` in `oxide86.log`.

**Important:** If you need to determine what wrote to a specific address but don't yet have watchpoints set, **stop and ask the user to re-run the program with the appropriate `--watch` flag** rather than trying to infer it from static analysis. The watchpoint log is the authoritative answer.

## MCP Debug Server

The emulator exposes a live debug interface over HTTP when started with `--debug-mcp <PORT>`.
Use this to inspect a running emulator session interactively rather than relying solely on static
analysis or log files.

### Starting the server

```bash
cargo run -p oxide86-native-gui -- --debug-mcp 7777 --boot --floppy-a game.img
```

Add `--debug-mcp-pause-on-start` to halt emulation on the very first instruction so you can inspect state before any code runs:

```bash
cargo run -p oxide86-native-gui -- --debug-mcp 7777 --debug-mcp-pause-on-start --boot --floppy-a game.img
```

### Registering with Claude Code (one-time per project)

```bash
claude mcp add --transport http oxide86 http://127.0.0.1:7777/mcp
```

Then run `/mcp` in the chat panel and click **Reconnect**.

### Available tools

| Tool | When to use |
|---|---|
| `get_status` | Check whether the emulator is running or paused |
| `get_registers` | Read all CPU registers (requires pause first) |
| `get_fpu_registers` | Read FPU stack ST(0)–ST(7), control word, and status word (requires pause) |
| `pause` / `continue` | Halt and resume execution |
| `step` | Execute N instructions while paused |
| `set_breakpoint` / `clear_breakpoint` / `list_breakpoints` | Breakpoints by CS:IP |
| `read_memory` | Dump bytes from a physical address (requires pause) |
| `set_write_watchpoint` / `clear_write_watchpoint` / `list_write_watchpoints` | Pause on memory writes |
| `send_key` | Inject a PC scan code into the keyboard buffer |

### Debugging workflow

When investigating a bug in a running program:
1. Start the emulator with `--debug-mcp 7777` and register the server (if not already done).
2. Use `set_breakpoint` or `set_write_watchpoint` to stop at the relevant point.
3. Use `get_registers` and `read_memory` to inspect state.
4. Use `step` / `continue` to advance execution.

**Prefer the MCP server over static analysis when possible** — it gives authoritative runtime state.
For write watchpoints you can also use the `--watch` CLI flag (logs to `oxide86.log` without
pausing), but `set_write_watchpoint` via MCP pauses execution so you can inspect registers immediately.

## Resources
- [8086 User Manual](https://edge.edx.org/c4x/BITSPilani/EEE231/asset/8086_family_Users_Manual_1_.pdf)
- [x86 Reference](https://www.felixcloutier.com/x86/)
- [8086 Opcodes](http://www.mlsite.net/8086/)
