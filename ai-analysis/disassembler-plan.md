# Plan: oxide86-disasm — 286 Disassembler

## Context

The emulator already has a rich instruction decoder (`core/src/cpu/instructions/decoder.rs`) tightly coupled to `Bus`. This plan extracts that logic into a reusable public API and builds a standalone `oxide86-disasm` binary that performs recursive-descent disassembly of COM and EXE files.

---

## Step 1 — Create workspace member `disassembler/`

**Files to create:**
- `disassembler/Cargo.toml`
- `disassembler/src/main.rs` (skeleton only)

**Cargo.toml:**
```toml
[package]
name = "oxide86-disasm"
version = "0.1.0"
edition = "2021"

[dependencies]
oxide86-core = { path = "../core" }
anyhow = { workspace = true }
log = { workspace = true }
env_logger = { workspace = true }
clap = { workspace = true, features = ["derive"] }
```

Add `"disassembler"` to workspace `members` in root `Cargo.toml`.

---

## Step 2 — CLI argument parsing

In `disassembler/src/main.rs`:

```rust
#[derive(Parser)]
struct Args {
    /// File to disassemble (.com or .exe)
    file: PathBuf,

    /// Additional entry points in SEG:OFF format (hex, e.g. 0000:01A0); may repeat.
    /// Use addresses exactly as they appear in emulator execution logs.
    #[arg(short, long, value_name = "SEG:OFF")]
    entry: Vec<String>,
}
```

**Parsing `SEG:OFF`:**
- Accept both `0000:01A0` and `0x0000:0x01A0` forms
- Split on `:`, parse each half as `u16` from hex
- Compute linear address as `(seg << 4) + off` for indexing into the loaded image
- Print a clear error if the format is wrong

**Default entry points (if no `--entry` given):**
- COM: `0x0000:0x0100`
- EXE: `cs_init:ip_init` from the MZ header

Parse and validate the file exists. Print usage on error.

---

## Step 3 — COM and EXE file parsing

In `disassembler/src/loader.rs`:

### COM
- Load raw bytes into a `Vec<u8>`
- Entry point: linear address `0x0100` (segment `0x0000`, offset `0x0100`)
- The entire file is the code/data image starting at offset 0x100

### MZ EXE
Parse the MZ header (first 28+ bytes):
```
magic       : u16  = 0x5A4D
last_page   : u16   (bytes in last page)
pages       : u16   (512-byte pages including last)
reloc_count : u16
header_size : u16   (in 16-byte paragraphs)
...
cs_init     : i16   (initial CS, relative to load segment)
ip_init     : u16   (initial IP)
```
- Header size in bytes = `header_size * 16`
- Load image starts at that offset in the file
- Entry point = `(cs_init as u16, ip_init)` — assume load segment 0x0000 for disassembly purposes (no relocation needed since we're doing static analysis)
- Skip relocation table (segment fixups are irrelevant for a flat-image disassembly)

**Output from loader:**
```rust
pub struct LoadedImage {
    pub data: Vec<u8>,           // flat byte image
    pub load_offset: usize,      // byte offset within image where code starts (always 0 here)
    pub entry_cs: u16,
    pub entry_ip: u16,
    pub kind: ImageKind,         // Com | Exe
}
```

---

## Step 4 — Decouple decoder from Bus

In `core/src/cpu/instructions/decoder.rs`:

### 4a — Add `ByteReader` trait to core (in `core/src/lib.rs` or a new `core/src/dis.rs`)

```rust
pub trait ByteReader {
    fn read_u8(&self, addr: usize) -> u8;
}
```

Implement for `Bus` (trivially — delegates to existing `memory_read_u8`).

### 4b — Make `InstructionDecoder` generic

Change:
```rust
struct InstructionDecoder<'a> {
    bus: &'a Bus,
    ...
}
```
To:
```rust
struct InstructionDecoder<'a, R: ByteReader> {
    reader: &'a R,
    ...
}
```

Replace all `self.bus.memory_read_u8(addr)` calls with `self.reader.read_u8(addr)`.

### 4c — Add 286 instructions to the `decode()` match

Missing opcodes to add:
| Opcode | Instruction |
|--------|-------------|
| 0x60   | `pusha` |
| 0x61   | `popa` |
| 0x62   | `bound r16, m16&16` (decode ModRM) |
| 0x68   | `push imm16` |
| 0x6A   | `push imm8` (sign-extended) |
| 0x69   | `imul r16, r/m16, imm16` |
| 0x6B   | `imul r16, r/m16, imm8` |
| 0x6C   | `insb` |
| 0x6D   | `insw` |
| 0x6E   | `outsb` |
| 0x6F   | `outsw` |
| 0xC8   | `enter imm16, imm8` |
| 0xC9   | `leave` |

### 4d — Add public disassembly API to core

New public types in `core/src/dis.rs` (or expose from decoder module):

```rust
pub enum FlowKind {
    Continue,
    Jump(u16),             // unconditional near jump to abs offset
    JumpFar(u16, u16),     // far jump (seg, off)
    ConditionalJump(u16),  // conditional — also falls through
    Call(u16),             // near call
    CallFar(u16, u16),     // far call
    Return,                // ret / retf / iret
    Halt,                  // hlt
    IndirectTransfer,      // jmp/call via register or memory
}

pub struct DisasmResult {
    pub text: String,
    pub bytes: Vec<u8>,
    pub next_ip: u16,
    pub flow: FlowKind,
}

pub fn disasm_one(reader: &impl ByteReader, cs: u16, ip: u16) -> DisasmResult;
```

The `FlowKind` is computed by inspecting the decoded instruction text or by augmenting the existing decoder to return control-flow metadata alongside the text. Simplest: pattern-match on the decoded string in `disasm_one` (e.g. text starts with `"jmp"`, `"call"`, `"ret"`, etc.) to determine `FlowKind` and extract the target offset.

---

## Step 5 — Implement recursive descent engine

In `disassembler/src/disasm.rs`:

```rust
pub struct Disassembly {
    pub instructions: BTreeMap<u16, DisasmEntry>,  // keyed by IP offset
    pub labels: BTreeSet<u16>,                      // IPs that need a label
}

struct DisasmEntry {
    pub cs: u16,
    pub ip: u16,
    pub result: DisasmResult,
}
```

**Algorithm:**
```
worklist: Vec<u16> = [entry_ip, ...additional_entries]
visited: HashSet<u16>

while let Some(ip) = worklist.pop():
    loop:
        if visited.contains(ip): break
        visited.insert(ip)

        result = disasm_one(reader, cs, ip)
        instructions.insert(ip, entry)

        match result.flow:
            Continue          => ip = result.next_ip
            Jump(target)      => labels.insert(target); ip = target
            ConditionalJump(t)=> labels.insert(t); worklist.push(t); ip = result.next_ip
            Call(target)      => labels.insert(target); worklist.push(target); ip = result.next_ip
            CallFar/JumpFar   => labels.insert(off); worklist.push(off); break
            Return|Halt       => break
            IndirectTransfer  => break
```

After the worklist is exhausted, any byte ranges not covered by instructions are treated as data.

---

## Step 6 — Output formatting

In `disassembler/src/output.rs`:

Walk all byte offsets in order. For each offset:

- If it's a label target → print `loc_XXXX:` (or `sub_XXXX:` if reached via CALL)
- If it's a decoded instruction → print:
  ```
  0000:0100  55 89 E5     push bp
  ```
  Format: `{cs:04X}:{ip:04X}  {bytes_hex:<12}  {mnemonic}`
  - bytes hex: space-separated, left-padded to 12 chars (fits up to 4 bytes neatly, wraps for longer)
- If it's uncovered data bytes → print as:
  ```
  0000:0150  41           db 0x41    ; 65 'A'
  ```
  For consecutive uncovered bytes, emit one `db` per byte.

### String detection (future-ready)
Structure the data-byte output through a `classify_data(bytes: &[u8], offset: usize) -> DataKind` function stub:
```rust
enum DataKind {
    Bytes(Vec<u8>),
    AsciiString(String),
}
```
Initially always returns `Bytes`. Later: detect runs of printable ASCII (len ≥ 4) and emit as `db "hello", 0x00`.

---

## Step 7 — Wire up main

```
main()
  └─ parse args
  └─ load file → LoadedImage
  └─ build SliceReader from LoadedImage::data
  └─ collect entry points (LoadedImage entry + --entry args)
  └─ run recursive_descent → Disassembly
  └─ format_output → print to stdout
```

---

## Files to create / modify

| Action | File |
|--------|------|
| Create | `disassembler/Cargo.toml` |
| Create | `disassembler/src/main.rs` |
| Create | `disassembler/src/loader.rs` |
| Create | `disassembler/src/disasm.rs` |
| Create | `disassembler/src/output.rs` |
| Create | `core/src/dis.rs` (public disasm API + ByteReader trait) |
| Modify | `core/src/cpu/instructions/decoder.rs` (make generic, add 286 opcodes) |
| Modify | `core/src/lib.rs` (expose `dis` module and `ByteReader`) |
| Modify | `Cargo.toml` (add `"disassembler"` to workspace members) |

---

## Verification

```bash
# Build only the disassembler
cargo build -p oxide86-disasm

# Disassemble a COM file
cargo run -p oxide86-disasm -- path/to/test.com

# Disassemble an EXE with an extra entry point (seg:off from emulator log)
cargo run -p oxide86-disasm -- path/to/test.exe --entry 0000:01A0

# Run full pre-commit (ensures core still compiles for all targets)
./scripts/pre-commit.sh

# Run existing tests (ensures decoder changes didn't break emulator)
cargo test --all
```

---

## Future hooks (design-ready, not implemented)

- **String detection**: `classify_data()` stub in `output.rs` — just return `Bytes` for now, replace later
- **Additional entry points**: `--entry SEG:OFF` flag already in CLI args; wired into worklist — use addresses directly from emulator execution logs
- **Symbol names**: Labels use `loc_XXXX` / `sub_XXXX` based on whether first reference was a JMP or CALL
