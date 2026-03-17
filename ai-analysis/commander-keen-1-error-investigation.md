# Commander Keen 1 — "Error during code expansion!" Investigation

## Overview

When running Commander Keen 1 in the emulator, the program prints:

```
Error during code expansion!
```

This document traces the exact code path that produces this error, as observed in `oxide86.log` around line 2,886,453.

## Program Loading

Commander Keen 1 is packed with LZEXE. On startup:

1. The LZEXE decompressor stub runs in segment `28A4`, decompressing the executable body into memory.
2. At approximately log line 763,155, the decompressed program begins executing at `102A:0000`.

## Error Trigger Location

The error originates inside the GIF/LZW image decoder at `102A:6BAE`:

```asm
102A:6BAB  3D A0 0F   cmp ax, 0x0FA0   ; compare output index to 4000
102A:6BAE  7C 12      jl  0x6bc2       ; continue if still under limit
102A:6BB0  B8 53 28   mov ax, 0x2853   ; fall through → load error format string
102A:6BB3  50         push ax
102A:6BB4  E8 7E 5D   call 0xc935      ; call printf-like formatter
```

`DI` is the output byte counter. When it reaches `0x0FA0` (4000 decimal), the `jl` is no longer taken and execution falls through to the error path.

## GIF/LZW Decoder Structure

The decoder is split across two functions:

| Address | Role |
|---------|------|
| `102A:6A50` | Outer loop — drives decoding, calls `6B62` once per LZW code |
| `102A:6B62` | Inner function — follows a single LZW code chain, writing output bytes |

The inner function (`6B62`) entry:

```asm
102A:6B62  push bp
102A:6B63  mov  bp, sp
102A:6B65  push si
102A:6B66  push di
102A:6B67  mov  si, [0xff36]   ; starting address from caller
102A:6B6A  xor  di, di         ; DI = 0 (output byte counter)
102A:6B6C  jmp  0x6bc2         ; enter loop
```

Inside the loop, for each decoded byte:

```asm
102A:6BA8  mov  ax, di         ; ax = current output index
102A:6BAA  inc  di
102A:6BAB  cmp  ax, 0x0FA0     ; 4000 = buffer limit
102A:6BAE  jl   0x6bc2         ; ok → write byte
            ; fall through → error
```

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

The compressed data being expanded comes from **EGAGRAPH.CK1** — the Commander Keen 1 graphics archive. This was established by tracing all DOS file I/O calls in segment `102A` from program start through the decoder overflow.

### Filename construction

The game assembles filenames at `232F:FF90` using three string parts:
- A base path from `232F:254C` (empty in this run)
- A stem string from `232F:2871` (e.g. `"EGAHEAD."` or `"EGAGRAPH."`)
- An extension from `232F:27C0` (`"CK1"`)

The stem at `232F:2871` had these bytes decompressed by the LZEXE stub:

| Linear Address | Byte | Char |
|---------------|------|------|
| `0x25B61` | `0x45` | `E` |
| `0x25B62` | `0x47` | `G` |
| `0x25B63` | `0x41` | `A` |
| `0x25B64` | `0x48` | `H` |
| `0x25B65` | `0x45` | `E` |
| `0x25B66` | (not read in log) | `A` (inferred) |
| `0x25B67` | (not read in log) | `D` (inferred) |
| `0x25B68` | `0x2E` | `.` |
| `0x25B69` | `0x00` | null |

This confirms the first file is `EGAHEAD.CK1` (7-char stem). The second data file opens with an 8-char stem (dot at `FF98` rather than `FF97`), consistent with `EGAGRAPH.CK1`.

### File I/O call sequence

Two open code paths were identified in segment `102A`:

| Address | Purpose |
|---------|---------|
| `102A:D5EF` | Opens the file for header/seek operations; handle stored in `[0xff40]` or `[0xff4a]` |
| `102A:5CE1` | `mov ax, 0x3D00` / `int 0x21` — opens for bulk read; handle stored in `[0xff48]` |

Full sequence (all in segment `102A`):

| Log Line | Operation | File | Notes |
|----------|-----------|------|-------|
| 785,295 | AH=3D open (`D5EF`) | EGAHEAD.CK1 | First open; 7-char stem, dot at `FF97` |
| 790,615 | AH=3E close (`CF8B`) | EGAHEAD.CK1 | Handle 5 closed via `[0xff4a]` |
| 790,936 | AH=3D open (`5CE4`) | EGAHEAD.CK1 | Reopened; `lseek(0, SEEK_END)` at `5CF4` to get file size |
| 794,988 | AH=3F read (`5D1D`) | EGAHEAD.CK1 | BX=5, CX=0xFFFF, DS:DX=3330:0000 — reads full header into memory |
| 797,345 | AH=3E close (`5D50`) | EGAHEAD.CK1 | |
| 847,789 | AH=3D open (`D5EF`) | EGAGRAPH.CK1 | 8-char stem, dot at `FF98` |
| 851,818 | AH=3F read (`D6E7`) | EGAGRAPH.CK1 | BX=5, CX=4 — reads 4-byte chunk count/header |
| 852,862 | AH=3F read (`D6E7`) | EGAGRAPH.CK1 | Second small read |
| 853,395 | AH=3E close (`CF8B`) | EGAGRAPH.CK1 | |
| 854,194 | AH=3D open (`D5EF`) | EGAGRAPH.CK1 | Reopened to seek to specific chunk offset |
| 858,809 | AH=3E close (`CF8B`) | EGAGRAPH.CK1 | |
| 859,560 | AH=3D open (`5CE4`) | EGAGRAPH.CK1 | Final open; followed by seek then bulk read |
| 863,719 | AH=3F read (`5D1D`) | **EGAGRAPH.CK1** | BX=5, CX=0xFFFF — **reads compressed chunk data** |
| 869,953 | LZW inner fn entry | — | `102A:6B62`, DI=0 |

The read at log line 863,719 supplies the compressed data to the LZW decoder. The decoded chunk expands beyond 4000 bytes, triggering the overflow.

## Root Cause

The 4000-byte output limit (`0x0FA0`) in the LZW decoder at `102A:6BAB` is a fixed buffer size (80×50 = 4000 bytes, matching a CGA text-mode screen). The chunk from `EGAGRAPH.CK1` being decoded at this point decompresses to more than 4000 bytes, overflowing the hard-coded buffer and triggering the error message.

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

### 4. INT 0x21 handler summary table

A compact optional mode that logs DOS calls as a one-liner table:

| # | Func | Args | Result |
|---|------|------|--------|
| 1 | open | "EGAHEAD.CK1" r/o | hnd=5 |
| 2 | close | hnd=5 | ok |
| 3 | open | "EGAHEAD.CK1" r/o | hnd=5 |
| 4 | read | hnd=5 65535B→3330:0000 | 1024B |

### 5. Named-interrupt annotation in oxide86.asm

The reverse-engineering output (`oxide86.asm`) emits raw `int 0x21` instructions. Post-processing or inline annotation based on the AH value just before the INT would make the disassembly far more readable:

```asm
102A:5CE4  CD 21  int 0x21   ; AH=3D → open file
102A:D5F1  CD 21  int 0x21   ; AH=3D → open file
102A:5D1F  CD 21  int 0x21   ; AH=3F → read file
```

### 6. DOS file handle tracking in the emulator

Maintain a small side-table mapping open file handles to their filenames and open positions. This would let any log line that shows `BX=5` be cross-referenced to the currently open file without manual reconstruction.

## Log Line Reference

| Log Line | Event |
|----------|-------|
| ~276,124 | LZEXE packer stub (seg `28A4`) begins processing |
| 763,155 | Keen starts executing at `102A:0000` |
| 785,295 | First open: `EGAHEAD.CK1` via `102A:D5EF` |
| 790,936 | Second open: `EGAHEAD.CK1` via `102A:5CE4` (with lseek for size) |
| 794,988 | Read: `EGAHEAD.CK1` chunk headers into `3330:0000` |
| 847,789 | Open: `EGAGRAPH.CK1` via `102A:D5EF` |
| 859,560 | Final open: `EGAGRAPH.CK1` via `102A:5CE4` |
| 863,719 | Read: compressed chunk from `EGAGRAPH.CK1` → fed to LZW decoder |
| 869,953 | First call to LZW decoder inner function (`102A:6B62`), DI=0 |
| ~2,885,297 | DI reaches `0x0FA0` (4000), overflow check fails |
| ~2,885,300 | Error format string assembled into stack buffer |
| ~2,886,171 | `INT 0x21 AH=40h` writes "Error during code expansion!\r\n" to stdout |
| 2,886,453 | First character `'E'` output via `INT 0x10 AH=0Eh` through `sub_055E3` |
