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

## Reverse Engineering Mode

Runtime-driven disassembly that builds a deduplicated map of all executed instructions and data reads, then emits a clean `.asm` file. Unlike the static disassembler (`oxide86-disasm`), this is driven by actual execution, so it only includes code paths that ran.

### Key files

| File | Role |
|------|------|
| `core/src/reverse_engineer.rs` | `ReverseEngineer` struct — records instructions, control-flow targets, and data reads; generates `.asm` via `to_asm_string()` |
| `core/src/bus.rs` | `data_reads_recorder: Option<Vec<(usize,u8)>>` — opt-in per-step read capture; enabled/drained by `Cpu::step()` |
| `core/src/cpu/mod.rs` | `reverse_engineer: Option<ReverseEngineer>` field; integrated in `step()` alongside exec logging |
| `core/src/computer.rs` | `set_reverse_engineer_enabled`, `reverse_engineer_enabled`, `get_reverse_engineer_asm` |
| `native-common/src/cli.rs` | `--reverse-engineer` flag |
| `native-cli/src/main.rs` | Applies flag; writes `oxide86.asm` on exit |
| `native-cli/src/command_mode.rs` | `re` command — toggles RE and writes `oxide86.asm` on disable |
| `native-gui/src/menu.rs` | `ToggleReverseEngineer` menu action; Debug → Reverse Engineering checkbox |
| `native-gui/src/main.rs` | Handles toggle; writes `oxide86.asm` on disable or window close |

### How it works

- **Instruction recording**: `cpu::step()` calls `disasm_one(bus, cs, ip)` before executing each instruction. `ReverseEngineer::record_instruction` stores the first-seen text/bytes at each linear address (deduplicates loops). Control-flow targets (`Call`, `Jump`, etc.) are tracked to generate labels.
- **Data read recording**: Bus read recording is enabled before `exec_instruction` and drained after. Reads in the instruction fetch range (`code_start..code_end`) and above `0xA0000` are filtered out, leaving only true data accesses.
- **`record_data_read(addr, val, ds)`**: stores `(val, ds-relative offset)` for each unique address; the offset is used to substitute `[0xNNNN]` patterns in instruction text with `data_XXXXX` labels.
- **Output (`to_asm_string()`)**: emits `entry:` for the first executed address, `sub_XXXXX:` for call targets, `loc_XXXXX:` for jump targets, and a `; Data` section with `db` lines (ASCII comment for printable bytes). Blank lines separate non-consecutive address ranges.

### Output path

Always `oxide86.asm` in the working directory. No configurable path.

### CLI usage

```bash
cargo run -p oxide86-cli -- --reverse-engineer myprogram.exe
# oxide86.asm written on exit
```

Or toggle live in command mode (F12): type `re` to enable, `re` again to disable and write the file.

## Resources
- [8086 User Manual](https://edge.edx.org/c4x/BITSPilani/EEE231/asset/8086_family_Users_Manual_1_.pdf)
- [x86 Reference](https://www.felixcloutier.com/x86/)
- [8086 Opcodes](http://www.mlsite.net/8086/)
