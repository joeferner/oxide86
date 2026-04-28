# logs-to-asm

Converts an oxide86 execution log into an annotated assembly listing. Each unique instruction is shown once with an execution count and its source address and bytes.

## Usage

```bash
cargo run -p oxide86-tools --bin logs-to-asm -- --out listing.asm [--log-file oxide86.log] [--config config.json]
```

Arguments:
- `--log-file` — defaults to `oxide86.log`
- `--out` — required; path to write the output file
- `--config` — optional JSON config file for annotations
- `--hot-threshold` — execution count at or above which an instruction is flagged `[HOT]`; defaults to `1000`

## Output format

```
   mov [bp-0x04], al     ;  45 -- 0019:06BA 88 46 FC
```

- 3-space indent
- disassembly left-aligned in a 24-char column
- `; <count> -- <SEG:OFF> <bytes>`
- instructions at or above `--hot-threshold` executions are marked `[HOT]` immediately after the count:

```
   in al, dx               ; 402735 [HOT] -- 0C45:22F6 EC
```

### Labels

Function entry points (call targets and `retf_targets`) get a label above the first instruction:

```
func_0019_423F:
   push bp               ;   1 -- 0019:423F 55
```

`call` instructions include the target label in the comment:

```
   call 0x423f           ;  func_0019_423F   1 -- 0019:43E4 E8 8E FE
```

If the target has a named label from the config, that name is used instead:

```
   call 0x423f           ;  func_write_screen   1 -- 0019:43E4 E8 8E FE
```

Jump targets (jmp, jcc, loop, jcxz) that are not also call targets get a `lbl_` label:

```
lbl_0019_4300:
   mov ax, 0x01          ;   3 -- 0019:4300 B8 01 00
```

Jump instructions include the target label in the comment:

```
   jne 0x4300            ;  lbl_0019_4300   5 -- 0019:42FA 75 04
```

If the jump target is also a call target, the `func_` label is used instead of `lbl_`.

Interrupt handler entry points get an `int_NNh:` label:

```
int_21h:
   push ax               ; 120 -- 0070:1234 50
```

## Config file

An optional JSON file annotates specific functions and instructions.

### `comment`

A top-level string appended to the output file immediately after the generated header. Use it to record a high-level description of the program or analysis notes. Multi-line strings (using `\n`) are supported; each line is emitted as a `;`-prefixed comment.

### `functions`

Keyed by `"SEG:OFF"`. Replaces the generated `func_SSSS_OOOO:` label with a human-readable name and optional comment block.

Both fields are optional. If `label` is omitted, the auto-generated name is used. If `comment` is omitted, no comment block is printed.

### `labels`

Same shape as `functions`, but for jump targets (addresses reached only by jmp/jcc/loop/jcxz, not by `call`). Replaces the generated `lbl_SSSS_OOOO:` label with a human-readable name and optional comment block.

If a jump target is also a call target, the `functions` entry takes precedence and the `labels` entry is ignored.

### `retf_targets`

Same shape as `functions`, but for addresses that are entered via a RETF-based longjmp trick rather than a normal `call`. These addresses do not appear in `call_targets` (because no `call` instruction targets them in the log), so they would otherwise be labelled as jump targets or not labelled at all.

Entries in `retf_targets` receive a `func_` label (or a custom `label`/`comment`) exactly like `functions` entries. If the same address appears in both `functions` and `retf_targets`, `functions` takes precedence.

Jump instructions (`jmp`, `jcc`, etc.) that target a `retf_targets` address are also annotated with the `func_` label in their comment, matching the behaviour for `call_targets`.

### `lineComments`

Keyed by `"SEG:OFF"`. Inserts a comment line immediately before the instruction.

### `gaps`

Keyed by `"SEG:OFF"` of the gap's start address. Appends an annotation to the `; gap` line that appears between executed blocks:

```
   ; gap 0C45:2B79 - 0C45:2B80 (7 bytes) remaining 7 bytes of 8-byte OEM ID comparison loop
```

### `memLabels`

Keyed by `"SEG:OFF"`. Names a well-known memory address so that instructions containing a direct `[0xNNNN]` reference to that offset (within the same segment) are annotated inline:

```
   mov [0x0082], 0x83    ;    1 -- 0C45:21A0 C6 06 82 00 83  cmd_code  
   mov dx, [0x0076]      ;    3 -- 0C45:21C4 8B 16 76 00     base_port  
```

### `ports`

Keyed by port number (up to 4 hex digits, case-insensitive). Names well-known I/O ports so that `in`/`out` instructions are annotated with the port's meaning.

For immediate ports (`in al, 0x21`) the port number is taken directly from the disassembly. For DX ports (`in al, dx` / `out dx, al`) the port is resolved from the `DX=NNNN` register annotation captured in the log:

- Consistent port in config → port name shown in comment
- Consistent port **not** in config → port number shown (`port 0xNNNN`) — useful when DX is not visible in the disassembly
- Port varies across calls → `port varies` shown in comment

```
   out dx, al            ;  SB-CD base+0 (cmd/result)   7 -- 0C45:2183 EE
   in al, dx             ;  SB-CD base+1 (busy flag)    2 -- 0C45:229F EC
   in al, dx             ;  port varies                  3 -- 0C45:22A1 EC
   in al, 0x21           ;  PIC1 data                   1 -- 0019:0100 E4 21
```

### `data`

Keyed by `"SEG:OFF"`. Labels known data regions (strings, byte tables, etc.) that live at addresses never executed as code. These appear as `; gap` blocks without annotation; the `data` section makes them readable.

Each entry has:
- `label` (required) — the name emitted as a label above the gap region
- `type` — `"bytes"` (default) or `"string"`
- `length` — optional byte count (shown in the label line for `"bytes"` entries)
- `comment` — optional comment printed above the label

The label is emitted at the start of the gap (or inline within a larger gap), splitting the gap line so the data region is clearly identified:

```
   ; gap 0C45:28E2 - 0C45:28EA (8 bytes)
; MATSHITA — Matsushita/MKE drive OEM string
expected_oem_id:   ; 0C45:28EA  bytes[8]
   ; gap 0C45:28F2 - 0C45:2962 (112 bytes)
str_not_ready:   ; 0C45:2962  string
```

Instructions that reference these addresses as plain immediates are annotated with the label name:

```
   mov si, 0x28ea        ;  expected_oem_id   1 -- 0C45:21B0 BE EA 28
   mov dx, 0x2962        ;  str_not_ready     1 -- 0C45:21C4 BA 62 29
```

Note: bracket references like `mov ax, [0x2962]` are handled by `memLabels` and are not annotated by `data`.

### Example

```json
{
  "comment": "CD-ROM driver — reverse-engineered from oxide86 execution log",
  "functions": {
    "0019:423F": {
      "label": "func_write_screen",
      "comment": "Updates the screen"
    },
    "0019:4000": {
      "label": "func_init"
    }
  },
  "retf_targets": {
    "36C5:06D2": {
      "label": "longjmp_handler",
      "comment": "Entered via RETF longjmp trick — not reachable by normal call"
    }
  },
  "labels": {
    "0019:4300": {
      "label": "loop_top",
      "comment": "Main draw loop entry"
    },
    "0019:4320": {
      "label": "skip_clear"
    }
  },
  "lineComments": {
    "0019:40EC": "compare screen offset",
    "0019:40EF": "jump if at end of line"
  },
  "gaps": {
    "0019:4310": "unreachable error path"
  },
  "memLabels": {
    "0019:0082": "cmd_code",
    "0019:0076": "base_port"
  },
  "ports": {
    "0230": "SB-CD base+0 (cmd/result)",
    "0231": "SB-CD base+1 (busy flag)",
    "0233": "SB-CD base+3 (drive select)"
  }
}
```

Produces output like:

```
func_write_screen:   ; 0019:423F
; Updates the screen
   push bp               ;   1 -- 0019:423F 55

func_init:   ; 0019:4000
   push bp               ;   1 -- 0019:4000 55

; Main draw loop entry
loop_top:   ; 0019:4300
   mov ax, 0x01          ;   3 -- 0019:4300 B8 01 00

skip_clear:   ; 0019:4320
   mov bx, 0x00          ;   2 -- 0019:4320 BB 00 00

   ; compare screen offset
   cmp ah, 0x6c          ;    1 -- 0019:40EC 80 FC 6C
   ; jump if at end of line
   jne 0x4300            ; loop_top   5 -- 0019:40EF 75 04
```

## LLM analysis instructions

When using this tool as part of an LLM-assisted reverse engineering workflow, follow these guidelines:

**Analyze one function at a time.** Pick a single function from the listing, understand it fully, then move on. You may examine call sites to understand how a function is used, but do not attempt to analyze the entire file in a single pass — the output is too large and context will be lost.

**Read the assembly header to find the config file.** The first few lines of the output file include the `--config` path used to produce it. Always read the header before making any edits so you know which config file to update.

**Preserve config order.** When writing entries to `config.json`, add `functions`, `labels`, and `lineComments` entries in the order you encounter them in the assembly listing (top to bottom by address). Do not sort or reorder existing entries.

**Use line comments for non-obvious instructions.** When an instruction has a purpose that is not immediately clear from the mnemonic alone — such as a magic constant comparison, a bitmask operation, or a loop boundary condition — add a `lineComments` entry with a short explanation. Reserve comments for lines that genuinely benefit from clarification; do not annotate every instruction.

## Tips

- Run the emulator with verbose CPU logging enabled so every executed instruction is captured in `oxide86.log`.
- Keys in the JSON config are case-insensitive (normalised to uppercase internally).
