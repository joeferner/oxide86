# Plan: Reverse Engineering Mode

## Context

The emulator's existing execution logging has drawbacks: it slows down execution, loops generate excessive noise, and repeated calls/data accesses are hard to reconstruct. Reverse Engineering mode addresses this by building a deduplicated map of all executed code and data accesses, then outputting a clean `.asm` file — similar in style to `oxide86-disasm` but driven by actual runtime behavior instead of static analysis.

## Key Design Decisions

- **`ReverseEngineer` struct lives in `Cpu`** — mirrors the `exec_logging_enabled` pattern. `cpu::step()` has direct access to `self.cs`, `self.ip`, and `bus`, so it can record everything without extra indirection.
- **`disasm_one(bus, cs, ip)`** is already public in `core/src/dis.rs` and `Bus` implements `ByteReader` (`bus.rs:312`), so we can decode instructions inside `cpu::step()` without a separate decode pass.
- **Data reads recorded via Bus** — add an opt-in recorder to Bus that captures `(addr, val)` pairs during `exec_instruction`. Instruction fetch bytes (CS:[pre_ip..new_ip]) are filtered out in `cpu::step()` after execution, leaving only true data reads.
- **Output via `to_asm_string()`** — keeps `core` free of file I/O (wasm-compatible). CLI/GUI write the file at the right moment.
- **Write triggers**: CLI writes on main-loop exit (program halt or user quit). GUI writes when RE is disabled or window closes. Command mode `re` command toggles and writes on disable.
- **Output path is always `oxide86.asm`** — no configurable path, no `output_path` field on Computer.

## Files to Modify/Create

| File | Change |
|------|--------|
| `core/src/reverse_engineer.rs` | **New** — `ReverseEngineer` struct and `.asm` generator |
| `core/src/lib.rs` | Add `pub mod reverse_engineer;` |
| `core/src/bus.rs` | Add `data_reads_recorder: Option<Vec<(usize,u8)>>` + `enable_read_recording` / `drain_read_recording`; record in `memory_read_u8` |
| `core/src/cpu/mod.rs` | Add `pub(crate) reverse_engineer: Option<ReverseEngineer>` field; integrate in `step()` |
| `core/src/computer.rs` | Add `set_reverse_engineer_enabled`, `reverse_engineer_enabled`, `get_reverse_engineer_asm` |
| `native-common/src/cli.rs` | Add `--reverse-engineer` flag |
| `native-cli/src/main.rs` | Apply CLI flag; write `oxide86.asm` on exit |
| `native-cli/src/command_mode.rs` | Add `re` command (toggle + auto-write on disable) |
| `native-gui/src/menu.rs` | Add `ToggleReverseEngineer` to `MenuAction`; add `reverse_engineer_enabled` state |
| `native-gui/src/main.rs` | Handle `ToggleReverseEngineer`; write `.asm` on disable or window close |

## Implementation Steps

### 1. `core/src/reverse_engineer.rs` (new)

```rust
pub struct ReverseEntry {
    pub cs: u16,
    pub ip: u16,
    pub text: String,
    pub bytes: Vec<u8>,
}

pub struct ReverseEngineer {
    instructions: BTreeMap<usize, ReverseEntry>,  // keyed by linear address
    call_targets: BTreeSet<usize>,
    jump_targets: BTreeSet<usize>,
    data_reads: BTreeMap<usize, u8>,  // linear addr → byte value
    entry_address: Option<usize>,
}
```

- `record_instruction(cs, ip, flow: &FlowKind, text, bytes)`:
  - Insert into `instructions` if not already present (first-seen wins, deduplication)
  - Set `entry_address` on first call
  - On `FlowKind::Call(target)` / `CallFar` → add to `call_targets` (uses CS to compute linear)
  - On `FlowKind::Jump/ConditionalJump(target)` → add to `jump_targets`
- `record_data_read(addr, val)`: insert into `data_reads` (first-seen wins)
- `to_asm_string() -> String`: generate the `.asm` output

**Output format** (matches `oxide86-disasm` style):

```asm
; Reverse Engineered by Oxide86

entry:
    1000:0100  55 89 E5              push bp
    1000:0103  E8 20 00              call sub_00125

sub_00125:
    1000:0125  55                    push bp
    ...
    1000:0145  C3                    ret

loc_00150:
    1000:0150  74 05                 jz 0x0157

; Data
data_00200:
    db 0x61    ;  97 'a'
    db 0x62    ;  98 'b'
```

Label naming (identical to disassembler logic):
- First executed address → `entry`
- Call targets → `sub_XXXXX` (5-digit linear hex)
- Jump targets → `loc_XXXXX`
- Data regions → `data_XXXXX`

Group consecutive data reads together under one label. Within a group, emit one `db` per byte with ASCII comment (byte value ≥ 0x20 && < 0x7F).

### 2. `core/src/bus.rs`

Add to `Bus` struct:
```rust
data_reads_recorder: Option<Vec<(usize, u8)>>,
```

Add methods:
```rust
pub(crate) fn enable_read_recording(&mut self) {
    self.data_reads_recorder = Some(Vec::new());
}
pub(crate) fn drain_read_recording(&mut self) -> Vec<(usize, u8)> {
    self.data_reads_recorder.take().unwrap_or_default()
}
```

In `memory_read_u8`, after determining `result`:
```rust
if let Some(ref mut rec) = self.data_reads_recorder {
    rec.push((addr, result));
}
```

### 3. `core/src/cpu/mod.rs`

Add field to `Cpu`:
```rust
pub(crate) reverse_engineer: Option<ReverseEngineer>,
```
Initialize to `None` in `Cpu::new()`.

In `step()`, after the BIOS check (line ~278), integrate alongside exec logging:
```rust
let pre_cs = self.cs;
let pre_ip = self.ip;

// Decode once for both exec logging and RE
let pre_decoded = if self.exec_logging_enabled || self.reverse_engineer.is_some() {
    Some(self.decode_instruction_with_regs(bus))
} else {
    None
};

// For RE: also get flow kind and enable bus read recording
let pre_disasm = if self.reverse_engineer.is_some() {
    bus.enable_read_recording();
    Some(crate::dis::disasm_one(bus, pre_cs, pre_ip))
} else {
    None
};

self.exec_instruction(bus);

// RE recording
if let (Some(ref mut re), Some(disasm)) = (&mut self.reverse_engineer, pre_disasm) {
    let all_reads = bus.drain_read_recording();
    let code_start = crate::physical_address(pre_cs, pre_ip);
    let code_end = crate::physical_address(self.cs, self.ip);
    for (addr, val) in all_reads {
        if addr < 0xA0000 && !(code_start..code_end).contains(&addr) {
            re.record_data_read(addr, val);
        }
    }
    let (text, bytes) = if let Some(ref d) = pre_decoded {
        (d.text.clone(), d.bytes.clone())
    } else {
        (disasm.text.clone(), disasm.bytes.clone())
    };
    re.record_instruction(pre_cs, pre_ip, &disasm.flow, text, bytes);
}

// ... existing exec logging code (unchanged) ...
```

### 4. `core/src/computer.rs`

Add methods:
```rust
pub fn set_reverse_engineer_enabled(&mut self, enabled: bool) {
    if enabled {
        self.cpu.reverse_engineer = Some(ReverseEngineer::new());
    } else {
        self.cpu.reverse_engineer = None;
    }
}

pub fn reverse_engineer_enabled(&self) -> bool {
    self.cpu.reverse_engineer.is_some()
}

pub fn get_reverse_engineer_asm(&self) -> Option<String> {
    self.cpu.reverse_engineer.as_ref().map(|re| re.to_asm_string())
}
```

### 5. `native-common/src/cli.rs`

Add to `CommonCli`:
```rust
/// Enable reverse engineering mode; writes oxide86.asm on exit
#[arg(long = "reverse-engineer")]
pub reverse_engineer: bool,
```

### 6. `native-cli/src/main.rs`

After creating computer:
```rust
if cli.common.reverse_engineer {
    computer.set_reverse_engineer_enabled(true);
}
```

After main loop exits (before cleanup):
```rust
if let Some(asm) = computer.get_reverse_engineer_asm() {
    std::fs::write("oxide86.asm", asm)?;
    eprintln!("Reverse engineer output written to oxide86.asm");
}
```

### 7. `native-cli/src/command_mode.rs`

Add `Command::ToggleReverseEngineer` variant.

In `Command::parse()`:
```rust
} else if text == "re" {
    Self::ToggleReverseEngineer
```

In the match handler:
- If disabling: get asm string, write to `oxide86.asm`, show message
- If enabling: call `set_reverse_engineer_enabled(true)`

Add to command list display: `"   re                      - Toggle reverse engineering [{}]\r"`.

### 8. `native-gui/src/menu.rs`

Add to `MenuAction`:
```rust
ToggleReverseEngineer,
```

Add `reverse_engineer_enabled: bool` to `AppMenu` struct and `update_debug_states` signature.

Add checkbox to Debug menu:
```rust
let mut b = self.reverse_engineer_enabled;
if ui.checkbox(&mut b, "Reverse Engineering").clicked() {
    action = Some(MenuAction::ToggleReverseEngineer);
    ui.close_menu();
}
```

### 9. `native-gui/src/main.rs`

Add `reverse_engineer_enabled: bool` to `AppState`.

Handle action:
```rust
MenuAction::ToggleReverseEngineer => {
    let enabled = !computer.reverse_engineer_enabled();
    if !enabled {
        // Write the file before disabling
        if let Some(asm) = computer.get_reverse_engineer_asm() {
            let path = "reverse_engineer.asm";
            if let Err(e) = std::fs::write(path, asm) {
                log::error!("Failed to write reverse engineer asm: {e}");
            } else {
                app_state.notification = Some(Notification::info(format!("Saved {path}")));
            }
        }
    }
    computer.set_reverse_engineer_enabled(enabled);
    app_state.reverse_engineer_enabled = enabled;
}
```

On window close / program halt, if RE is enabled, write `reverse_engineer.asm` (same pattern).

## Verification

1. Build: `./scripts/pre-commit.sh`
2. Test with a simple COM program:
   ```
   cargo run -p oxide86-cli -- --reverse-engineer /tmp/test.asm core/src/test_data/hello.com
   cat /tmp/test.asm
   ```
3. Verify: `entry:` label at program start, `sub_XXXXX:` labels for function calls, `loc_XXXXX:` for jump targets, `data_XXXXX:` sections with `db` entries.
4. Test CLI command mode: run emulator, press F12, type `re /tmp/out.asm`, resume, F12 again, type `re` to disable → file written.
5. Test GUI: Debug → Reverse Engineering checkbox on/off → `reverse_engineer.asm` appears.
6. Run `cargo test --all` to ensure no regressions.
