# Commander Keen 1 — "Error during code expansion!" Investigation

## Overview

When running Commander Keen 1 in the emulator, the program prints:

```
Error during code expansion!
```

This document traces the exact code path that produces this error, as observed in `oxide86.log` around line 2,886,453.

**Summary of findings:** There are two separate issues.

1. The game has a historical buffer-sizing bug (4,000-byte limit, sized for CGA, used for EGA data). On real hardware this limit is never reached because the maximum possible chain depth is 3,839 < 4,000.
2. oxide86 has a shift-by-CL bug (mod-16 masking) that corrupts the LZW bit stream from the second code onward. The garbled codes eventually produce a chain depth > 4,000, triggering the error in a way that never happens on real hardware.

## Program Loading

Commander Keen 1 is packed with LZEXE. On startup:

1. The LZEXE decompressor stub runs in segment `28A4`, decompressing the executable body into memory.
2. At approximately log line 763,155, the decompressed program begins executing at `102A:0000`.

## Error Trigger Location

The error originates inside the custom LZW decoder at `102A:6BAE`:

```asm
102A:6BAB  3D A0 0F   cmp ax, 0x0FA0   ; compare output index to 4000
102A:6BAE  7C 12      jl  0x6bc2       ; continue if still under limit
102A:6BB0  B8 53 28   mov ax, 0x2853   ; fall through → load error format string
102A:6BB3  50         push ax
102A:6BB4  E8 7E 5D   call 0xc935      ; call printf-like formatter
```

`DI` is the output byte counter. When it reaches `0x0FA0` (4000 decimal), the `jl` is no longer taken and execution falls through to the error path.

## Custom LZW Decoder Structure

The decoder is implemented as three cooperating routines:

| Address | Role |
|---------|------|
| `102A:6A50` | Outer loop — iterates one code per pass; manages previous-code state |
| `102A:6BD4` | Code-extraction function — refills bit buffer, extracts next variable-width code |
| `102A:6B62` | Chain-follow function — expands a single code to output bytes; DI = byte counter |

### Code-Extraction Function (`102A:6BD4`)

**Bit buffer** state lives in three DS-relative variables:

| Variable | Width | Role |
|----------|-------|------|
| `[0x82f2]` | 16-bit | Low 16 bits of 32-bit bit buffer |
| `[0x82f4]` | 16-bit | High 16 bits of 32-bit bit buffer |
| `[0x82f6]` | 16-bit | Number of valid bits currently held |

**Bit-fill loop** (`6BDB–6C0E`): while `[0x82f6] ≤ 24`, read one byte from `7D0F:offset` via the `E1B3` nibble-pointer-advance routine, shift it into the high bits of the 32-bit buffer via `E151`, and add 8 to `[0x82f6]`.

**Code extraction** (`6C15–6C43`):
1. `E192` right-shifts the 32-bit buffer by `(32 − code_bits)` to produce the code in `AX`.
2. `E151` left-shifts the 32-bit buffer by `code_bits` to consume those bits.
3. Subtract `code_bits` from `[0x82f6]`.

Confirmed from log (first iteration, oxide86):

```
[0x82f4]=E140 [0x82f2]=E062  [0x82f6]=0x1F (31 valid bits)
code_bits = [0x96fa] = 9
→ E192: shr E140:E062 by (32−9) = 23  →  AX = 0x01C2  (code 450)   ← garbled (oxide86 bug)
→ E151: shl 32-bit buffer left by 9   →  [0x82f4]=81C0 [0x82f2]=C400
→ [0x82f6] = 0x16 (22 bits remaining)
```

On real hardware the buffer at the same point would be `0x0040A070` (not `0x0070A070`) and the second code would be `0x0102 = 258`, which is a valid dictionary entry.

**Seed loading and the oxide86 shift bug** — The file header contains 4 seed bytes at offsets 6–9 (`00 40 A0 70`) loaded into the 32-bit buffer before decoding begins. The E151 routine shifts BX left by CL bits using the sequence:

```asm
neg  cl          ; negate shift count
add  cl, 16      ; add 16 to get complement
shr  bx, cl      ; shift BX right by complement to achieve left shift
```

For the 4th byte (0x70), the original shift is 0, so `neg 0 → 0`, `add 0+16 → 16`, `shr BX, 16`. On real 8086, shifting a 16-bit register by 16 produces 0. oxide86 incorrectly masks the shift count mod 16 (modern x86 behavior), so `shr BX, 16` becomes `shr BX, 0`, leaving BX = 0x70. The subsequent `or DX, BX` puts 0x70 into the high word, yielding buffer `0x0070A070` instead of `0x0040A070`.

The top 9 bits of both values are 0, so the **first code is identical** (0x000 = literal NUL). The second code onwards diverges because the buffered bits differ. oxide86 reads `0x01C2 = 450` as the second code; at that point only codes 0–256 are defined, so the decoder is processing an invalid code. Garbled codes eventually produce a chain depth > 4,000, triggering the error message.

### Chain-Follow Function (`102A:6B62`)

```asm
102A:6B62  push bp
102A:6B63  mov  bp, sp
102A:6B65  push si
102A:6B66  push di
102A:6B67  mov  si, [0xff36]   ; [bp+04] = output buffer address (from caller)
102A:6B6A  xor  di, di         ; DI = 0 (output byte counter)
102A:6B6C  jmp  0x6bc2         ; enter chain-follow loop

; Loop: if code > 0xFF, look up char/next-state tables, recurse
102A:6BC2  cmp  [0xff38], 0x00ff   ; [bp+06] = code to expand
102A:6BC7  ja   0x6b6e             ; code > 0xFF → follow table chain
; leaf: code ≤ 0xFF → write literal byte
102A:6BC9  mov  al, [0xff38]
102A:6BCC  mov  [si], al           ; write to output address
102A:6BCE  mov  ax, si             ; return output address
102A:6BD0  pop  di
102A:6BD1  pop  si
102A:6BD2  pop  bp
102A:6BD3  ret
```

When DI reaches `0x0FA0` (4000) inside the chain-follow loop the overflow check fires:

```asm
102A:6BAB  cmp  ax, 0x0FA0     ; 4000 = hard-coded buffer limit
102A:6BAE  jl   0x6bc2         ; ok → continue
            ; fall through → error path
```

### Compression Parameters

The file header's 2-byte type field (`0x000C = 12`) selects the decoder parameters:

| Parameter | Value |
|-----------|-------|
| Initial code width | 9 bits |
| Maximum code width | 12 bits (from `[0x8326]` = 12 = type field) |
| Code space | 0x000–0x0FF literals; 0x100 clear; 0x101 end; 0x102–0xFFF dictionary |
| Maximum dictionary code | 0xFFF = 4095 |
| Maximum chain depth | 4095 − 256 = **3,839** — safely under the 4,000-byte buffer |

**Code-width growth** — the current width in `[0x96fa]` starts at 9 and grows via a threshold mechanism:

- Threshold `[0x8348]` is initialised to `(1 << code_bits) − 1` = 511.
- Dict entries are added only while `next_code ≤ threshold` (`ja` check at `102A:6AEF`).
- When `next_code` (after increment) equals the threshold, code_bits is incremented and the threshold doubles minus one (log: `inc [0x96fa]` triggered when DI=0x01FF at `102A:6B37`).
- Thresholds: 511 → 1023 → 2047 → 4095; stops growing at 12 bits.

### Compression Format Classification

The algorithm is a **custom variable-width LZW variant** specific to the id Tech 0 / Commander Keen Vorticons engine (1990). It is **not** standard GIF LZW, TIFF LZW, or any other well-known variant:
- GIF LZW (LSB-first, min_code_size parameter): produces invalid output from this file.
- TIFF LZW (MSB-first): produces invalid output.
- Fixed-width 12-bit LZW: produces invalid output.

The custom nature is consistent with id Software's practice of writing proprietary compression for every asset type in this era (Carmack, RLEW, custom Huffman are used elsewhere in the same engine).

## Decoder Validation

A Python implementation (`ai-analysis/keen1_latch_decode.py`) was written to match the game's exact decoder logic — same bit-fill loop, same MSB-first accumulator, same threshold-based code-width growth — and run against the actual `EGALATCH.CK1` file:

```
File size (compressed) : 57,065 bytes
Total uncompressed     : 119,680 bytes
Decoded                : 119,680 bytes  ✓
Max chain depth seen   : 57  (limit: 4,000)
```

The maximum observed chain depth is 57, not 3,839 or anywhere near 4,000. This confirms that on correct 8086 hardware the overflow check at `102A:6BAB` is **never reached**. The historical 4,000-byte limit is safe for this file.

## Error Path

When the buffer overflows, `102A:6BB0` loads the format string pointer `0x2853` and calls a printf-like routine at `102A:C935`. That routine formats `"Error during code expansion!\r\n"` (30 bytes) into a stack buffer, then issues:

```
INT 0x21  AH=0x40  BX=1  CX=0x1E  DX=<buffer>
```

This is a DOS write-to-stdout call (handle 1, 30 bytes).

## DOS Output Path

The INT 0x21 AH=40h write is handled by the DOS kernel (segment `0019`), which routes each character through **INT 0x29** (fast console output):

```
0019:4121  ...
0019:5474  CD 29      int 0x29
```

The INT 0x29 handler at `0070:0762` calls **INT 0x10 AH=0Eh** (BIOS teletype output):

```
0070:076C  CD 10      int 0x10   ; AH=0Eh, AL=char
```

Each character is then printed one at a time via the BIOS video service. The first character printed is `'E'` (0x45), visible at log line 2,886,453:

```
INT 0x10 0x0E Teletype Output: ch:'E', page:0, cursor:0,10
```

## Connection to oxide86.asm `sub_055E3`

`sub_055E3` in `oxide86.asm` corresponds to the DOS kernel routine at `0019:5453`. It is the per-character output helper called from the INT 0x29 write path:

```asm
sub_055E3:
    0019:5453  push bx
    0019:5454  mov  bx, 0x0001
    0019:5457  call 0x756f       ; check output mode
    ...
    0019:5474  int  0x29         ; fast character output
```

Each of the 28 printable characters in `"Error during code expansion!"` passes through this routine.

## Source File Identification

The compressed data being expanded comes from **EGALATCH.CK1** — the Commander Keen 1 EGA latch data archive. This was established directly from the structured DOS file I/O log added to the emulator.

> **Note:** An earlier version of this document incorrectly identified the file as `EGAGRAPH.CK1`. That name never appears in the log. The structured `[DOS]` logging added as improvement #1 below made the correct file immediately visible.

### File I/O call sequence

All DOS file operations observed from Keen startup through the decoder overflow:

| Log Line | Operation | File | Notes |
|----------|-----------|------|-------|
| 795,629 | AH=3D open | EGAHEAD.CK1 | First open; seek to get size |
| 800,960 | AH=3E close | EGAHEAD.CK1 | |
| 801,279 | AH=3D open | EGAHEAD.CK1 | Reopened; `lseek(SEEK_END)` then `lseek(0)` for size |
| 807,698 | AH=3F read | EGAHEAD.CK1 | buf=3330:0000 max=65535 → read=15568 bytes — reads full header into memory |
| 807,707 | AH=3E close | EGAHEAD.CK1 | |
| 858,235 | AH=3D open | EGALATCH.CK1 | First open |
| 863,268 | AH=3F read | EGALATCH.CK1 | buf=232F:FF5A max=4 → read=4 (`80 D3 01 00`) — chunk count/header |
| 863,810 | AH=3F read | EGALATCH.CK1 | buf=232F:8326 max=2 → read=2 (`0C 00`) — second small read |
| 863,852 | AH=3E close | EGALATCH.CK1 | |
| 864,654 | AH=3D open | EGALATCH.CK1 | Reopened; seek to get file size |
| 869,281 | AH=3E close | EGALATCH.CK1 | |
| 870,033 | AH=3D open | EGALATCH.CK1 | Final open; seek to chunk offset then bulk read |
| ~875,000 | AH=3F read | **EGALATCH.CK1** | buf=7D0F:0004 max=65535 → **read=57,065 bytes** — compressed chunk data fed to LZW decoder |
| ~875,010 | AH=3E close | EGALATCH.CK1 | |
| ~876,000 | LZW inner fn entry | — | `102A:6B62`, DI=0 |

The read of 57,065 bytes supplies the compressed data to the LZW decoder. In oxide86 the garbled codes eventually produce a chain depth > 4,000, triggering the overflow.

## Root Cause

### Real hardware

The 4,000-byte output limit (`0x0FA0`) at `102A:6BAB` is a historical artifact: the buffer was sized for an 80×50 CGA screen (80×50 = 4,000 bytes) and was never updated when the engine was ported to EGA. However, the actual maximum chain depth achievable with the type-12 parameters is 3,839 (code 0xFFF − 256 literals), which is **below** the limit. The overflow check is never triggered on correctly functioning hardware.

### oxide86

The emulator's shift-by-CL implementation masked the CL shift count mod 16 (x86 486+ behaviour), treating `shr BX, 16` as `shr BX, 0`. This corrupted the LZW bit-buffer seed, producing an invalid second code (0x01C2 = 450 instead of 0x0102 = 258). The cascading effect of invalid codes caused chain depths exceeding 4,000, triggering the game's overflow check.

## Emulator Fix

The shift-by-CL bug was fixed in `core/src/cpu/instructions/shift_rotate.rs`:

- **CL fetch** (opcodes `D2`/`D3`): removed `& 0x1F` masking — the 8086 does not mask shift counts.
- **`shl_8` / `shr_8`**: changed `count > 8` to `count >= 8` so that shifting an 8-bit register by exactly 8 produces 0 (Rust's `wrapping_shr(8)` on `u8` would otherwise wrap mod 8 and return the original value).
- **`shl_16` / `shr_16`**: changed `count > 16` to `count >= 16` for the same reason.

A TDD test was added at `core/src/test_data/cpu/shift_cl.asm` covering:

| Test | CL | Input | Expected |
|------|----|-------|----------|
| `test_shr_cl_normal` | 4 | `0x00F0` | `0x000F` |
| `test_shl_cl_normal` | 4 | `0x000F` | `0x00F0` |
| `test_shr_cl_16` | 16 | `0x1234` | `0x0000` |
| `test_shl_cl_16` | 16 | `0x1234` | `0x0000` |
| `test_shr_cl_17` | 17 | `0xFFFF` | `0x0000` |
| `test_shl_cl_17` | 17 | `0xFFFF` | `0x0000` |
| `test_shr_cl_zero` | 0 | `0xABCD` | `0xABCD` |
| `test_shl_cl_zero` | 0 | `0xABCD` | `0xABCD` |
| `test_shr_byte_cl_8` | 8 | `0xFF` | `0x00` |
| `test_shr_byte_cl_9` | 9 | `0xFF` | `0x00` |

## Suggested Emulator Improvements

### ✅ 1. DOS INT 0x21 high-level logging

The most immediately useful improvement would be adding structured logging for DOS file operations inside the INT 0x21 handler. For each call, log the function name, the filename string (read from DS:DX as a null-terminated C string), and the returned handle. For example:

```
[DOS] AH=3D open  "EGAHEAD.CK1" mode=00 → handle=5
[DOS] AH=3F read  handle=5 buf=3330:0000 max=65535 → read=1024
[DOS] AH=3E close handle=5
[DOS] AH=42 seek  handle=5 origin=2 offset=0:0 → pos=48376
```

This would eliminate the need to trace dozens of individual instructions just to reconstruct a filename.

Implementation sketch:
- In the INT 0x21 dispatch (segment `0019:40EB`), hook at the Rust emulator level before entering the DOS kernel
- Read the filename string from guest memory at `DS:DX` using `bus.memory_read_u8` in a loop up to the null terminator
- Log with `log::info!` using a `[DOS]` prefix so it is easy to `grep`

### ✅ 2. INT 0x21 AH=3Fh buffer dump

When the emulator handles a successful `AH=3Fh` read, optionally log the first N bytes of the destination buffer. This would immediately reveal what data was read without needing to trace memory accesses later.

### ✅ 3. INT 0x10 / INT 0x29 character output grouping

Currently each character of `"Error during code expansion!\r\n"` produces a separate log line via `INT 0x10 AH=0Eh`. Buffering consecutive `AH=0Eh` calls and flushing them on a non-printable character (CR, LF, or a non-0Eh call) would collapse 28 lines into one:

```
[BIOS] INT10 teletype: "Error during code expansion!"
```

### ❌ 4. INT 0x21 handler summary table

A compact optional mode that logs DOS calls as a one-liner table:

| # | Func | Args | Result |
|---|------|------|--------|
| 1 | open | "EGAHEAD.CK1" r/o | hnd=5 |
| 2 | close | hnd=5 | ok |
| 3 | open | "EGAHEAD.CK1" r/o | hnd=5 |
| 4 | read | hnd=5 65535B→3330:0000 | 1024B |

### ❌ 5. Named-interrupt annotation in oxide86.asm

The reverse-engineering output (`oxide86.asm`) emits raw `int 0x21` instructions. Post-processing or inline annotation based on the AH value just before the INT would make the disassembly far more readable:

```asm
102A:5CE4  CD 21  int 0x21   ; AH=3D → open file
102A:D5F1  CD 21  int 0x21   ; AH=3D → open file
102A:5D1F  CD 21  int 0x21   ; AH=3F → read file
```

### ✅ 6. DOS file handle tracking in the emulator

Maintain a small side-table mapping open file handles to their filenames and open positions. This would let any log line that shows file handles be cross-referenced to the currently open file without manual reconstruction.

### ✅ 7. Fix shift-by-CL to not mask shift count (completed)

The 8086 shift instructions (`D2`/`D3`) must not mask CL. This was the root cause of the oxide86 crash described in this document. Fixed in `core/src/cpu/instructions/shift_rotate.rs`; verified by `core/src/test_data/cpu/shift_cl.asm`.

## Log Line Reference

| Log Line | Event |
|----------|-------|
| ~276,124 | LZEXE packer stub (seg `28A4`) begins processing |
| 763,155 | Keen starts executing at `102A:0000` |
| 795,629 | First open: `EGAHEAD.CK1` |
| 800,960 | Close: `EGAHEAD.CK1` |
| 801,279 | Second open: `EGAHEAD.CK1` (lseek for size, then bulk read) |
| 807,698 | Read: `EGAHEAD.CK1` → 15,568 bytes into `3330:0000` |
| 807,707 | Close: `EGAHEAD.CK1` |
| 858,235 | First open: `EGALATCH.CK1` |
| 863,268 | Read: `EGALATCH.CK1` → 4 bytes (chunk count header) |
| 863,852 | Close: `EGALATCH.CK1` |
| 864,654 | Second open: `EGALATCH.CK1` (seek for file size) |
| 869,281 | Close: `EGALATCH.CK1` |
| 870,033 | Third open: `EGALATCH.CK1` (seek to chunk offset, then bulk read) |
| ~875,000 | Read: `EGALATCH.CK1` → **57,065 bytes** into `7D0F:0004` — fed to LZW decoder |
| ~875,010 | Close: `EGALATCH.CK1` |
| ~876,000 | First call to LZW decoder inner function (`102A:6B62`), DI=0 |
| ~2,901,000 | DI reaches `0x0FA0` (4000), overflow check fails (oxide86 only — due to shift bug) |
| ~2,901,100 | Error format string assembled into stack buffer |
| ~2,902,100 | `INT 0x21 AH=40h` writes "Error during code expansion!\r\n" to stdout |
| 2,902,187 | First character `'E'` output via `INT 0x10 AH=0Eh` |
