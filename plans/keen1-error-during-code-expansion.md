# Commander Keen 1 - "Error during code expansion!" Investigation

## Error

When running Commander Keen 1, the game displays "Error during code expansion!".

## LZEXE 0.91 Compression Analysis

keen1.exe (51190 bytes) is LZEXE 0.91 compressed ("LZ91" at file offset 0x1C).

EXE Header:
- CS offset: 0x0C66, IP: 0x000E (entry at file offset 0xC68E)
- SS offset: 0x1892, SP: 0x0080
- Min para: 0x119F, Max para: 0xFFFF

LZEXE Stub (374 bytes at file offset 0xC680):
1. Entry CS:000E: copies 374-byte stub backward (STD) to ES = CS + 0x0C14
2. RETF to relocated stub at (CS+0x0C14):0x002B
3. Big copy: 0xC660 bytes backward from load_seg:0 to (load_seg+0x0C14):0
   - After copy: DS = load_seg+0x0C14 (compressed source), ES = load_seg (output)
4. CLD; XOR SI,SI; XOR DI,DI - forward decompression
5. Decompressor loop at 0x0069:
   - 16-bit control words in BP, bit counter in DX
   - CF=1 -> literal: MOVSB
   - CF=0, next_bit=0 -> short match: 2 bits length + 1 byte offset (BX=0xFF:al)
   - CF=0, next_bit=1 -> long match: 2-byte token, BH |= 0xE0, BH>>=3
     - if length==0: read extra byte; if 0=end, if 1=segment change
   - Back-ref copy: MOV AL,[ES:BX+DI]; STOSB; LOOP
6. Segment change (AL==1) at 0x00D1:
   - DI = (DI & 0xF) + 0x2000; ES = ES + (old_DI>>4) - 0x200
   - SI = (SI & 0xF);          DS = DS + (old_SI>>4)
   - Preserves physical address, normalizes DI into 0x2000..0x200F range
7. End of data (AL==0): jumps to relocation at 0xFC
8. Relocation at 0xFC:
   - PUSH CS; POP DS (DS = stub segment)
   - POP BX -> PSP segment; ADD BX,0x10 -> BX = load_seg (relocation base)
   - Reads relocation table from stub+0x158, adds load_seg to each entry

All LZEXE-relevant CPU instructions were verified correct:
- SHR BP,1 (D1ED): correctly shifts LSB into CF
- RCL CX,1 (D1D1): correctly shifts CF into CX
- LODSW/LODSB: correct DS:SI with segment override
- STOSB: correct ES:DI
- MOVSB/MOVSW: correct with direction flag
- MOV DS,AX / MOV ES,AX: immediate effect
- MOV AL,[ES:BX+DI] (26 8A 01): segment override correctly uses ES
- decode_modrm: [BX+DI] (R/M=001) correctly uses DS default, ES override
- memory write_u8: no address filtering that would block writes
- physical_address: (seg<<4)+offset correct

## Commander Keen's Internal Decompressor

After LZEXE, Commander Keen has its own state-machine decompressor at 0x0F65:6B6E:
- Reads from compressed chunk at 0x8CAD:0x0004 (physical 0x8CAD4)
- State table at 0x8A39:0x0004 (physical 0x8A394)
- State table root state = 0x05B6 at physical 0x8AF00
- Watchdog fires after 0x0FA0 (4000) stuck iterations -> "Error during code expansion!"
- Error: state 0x05B6 -> reads 0x00 from physical 0x8D08A -> stays at 0x05B6 -> loop

## Two-Stage Architecture

1. LZEXE decompresses the EXE image (game code + state tables + static data)
2. Commander Keen's loader loads .CK1 files from disk into memory, then decompresses
   them with its internal Carmack/Huffman decompressor

keen1.img contains all required .CK1 files:
- EGAHEAD.CK1 (15568), EGALATCH.CK1 (57065), EGASPRIT.CK1 (17633)
- SOUNDS.CK1 (8898), LEVEL01-LEVEL16.CK1, etc.

## Hypothesis for Previous Error

Commander Keen's loader couldn't find EGAHEAD.CK1 etc. -> zero data in compressed
buffer -> state machine reads zeros -> infinite loop -> watchdog -> error.

## Current Status - 2026-02-14 Investigation

Error still occurs when running KEEN1.EXE from booted DOS (hdd.img):
```bash
RUST_LOG=debug cargo run -p oxide86-native-gui -- --boot --hdd examples/hdd.img \
  --boot-drive 0x80 --cpu 286 --speed 20 --memory 2048 --floppy-a examples/keen1.img
```
Then: type "A:" and "keen1.exe"

### New Findings

**Decompressor Instance:**
- NOT the old LZEXE or initial KEEN decompressor from previous investigation
- Different instance: state table at 0x7F29:0x0004, decompressor at CS:IP 0455:6B6E
- Watchdog fires after 4000 iterations stuck at state entry physical 0x80010

**Root Cause Identified:**
- State table entry at 0x80010 should be **0x0B00** but contains **0x06BE**
- Incorrect value computed during KEEN's initialization by reading from 0x81F73 = 0x00 (uninitialized)
- Decompressor reads state 0x06BE → compressed data → stays in loop → watchdog → error

**File Loading (Verified Correct):**
- EGAHEAD.CK1 loaded via INT 13h to physical 0x713A4-0x7F1A4 (111 sectors, 56832 bytes)
- File contents verified correct (MD5 matches extracted EGAHEAD.CK1 from floppy)
- No additional files loaded to 0x80000-0x83000 region

**Initialization Order Bug:**
KEEN's initialization processes EGAHEAD.CK1 to build tables in 0x7F000-0x83000:

1. **Early phase (21:58:19.426):**
   - Reads from file start (0x713AA-0x713AF)
   - Reads from 0x81B96 → gets **0x00** (uninitialized) ❌
   - Writes 0x00 to 0x81AD7 (propagating incorrect value)
   - Reads from 0x81F73 → gets **0x00** (uninitialized) ❌

2. **Later phase (21:58:19.589):**
   - Reads from file offset 0xDA+ (0x71484-0x71487)
   - Computes and writes **0xF3** to 0x81B96 ✓ (too late!)

3. **Final phase (21:58:20.924):**
   - Reads from 0x81F73 = 0x00 (still uninitialized)
   - Computes and writes **0x06BE** to 0x80010 ❌ (should be 0x0B00)

**Memory Dump Analysis (ram.bin):**
- Final state shows 0x81B96 = 0xF3 (correct)
- Pattern f38fff99c3c399ff exists only at 0x81B96 (no segment aliasing)
- Proves initialization completes correctly, just in wrong order

**Timeline:**
- File load completes: 21:58:19.396
- First read from 0x81B96: 21:58:19.426 (30ms later) → gets 0x00
- Write to 0x81B96: 21:58:19.589 (193ms later) → writes 0xF3
- 406 KEEN operations occur between early read and late write

### Why It Fails

KEEN expects to read valid data from 0x81xxx region before that data is computed/written:
- **Expected:** Initialize 0x81Bxx → Read 0x81Bxx → Build state table
- **Actual:** Read 0x81Bxx (0x00) → Build state table (wrong) → Initialize 0x81Bxx (0xF3)

This works on real hardware/DOSBox but fails in our emulator, indicating an emulator-specific issue.

### Hypotheses

1. **LZEXE DATA segment:** KEEN1.EXE (after LZEXE decompression) might have initialized DATA segment with preset values for 0x81xxx region that isn't being loaded correctly

2. **DOS BSS initialization:** DOS might be supposed to pre-initialize the 0x81xxx region from KEEN's .EXE file, but our emulator or the DOS we're running doesn't do this

3. **Control flow bug:** Interrupt, timing, or branch issue causing initialization phases to execute out of order

4. **Segment calculation:** Memory addressing bug causing reads/writes to map to wrong physical addresses (less likely given ram.bin shows data at expected location)

### What Works

- ✅ LZEXE decompression (verified in previous investigation)
- ✅ File loading (EGAHEAD.CK1 loads correctly)
- ✅ Initialization logic (final memory state is correct)
- ✅ INT 13h disk I/O (CHS calculations, sector reads)

### What Doesn't Work

- ❌ Initialization order (reads happen before writes)
- ❌ 0x81xxx region pre-initialization (starts as zeros instead of expected values)

## Tested Hypotheses (Ruled Out) - 2026-02-14

1. ❌ **LZEXE DATA segment**: Verified LZEXE doesn't write to 0x80000-0x83000 region during decompression
2. ❌ **BSS garbage dependency**: Tested with non-zero memory initialization (incrementing pattern 0x00-0xFF) - still fails
   - With pattern: reads 0x46 instead of 0x00 from 0x81446 (address changes per run due to DOS allocation)
   - Proves KEEN reads whatever is in uninitialized memory, but needs specific correct values
3. ❌ **Previous run data**: Fresh install confirmed, no save files or cache
4. ❌ **File loading issues**: EGAHEAD.CK1 loads correctly, verified via MD5 checksum

## Complete Analysis - 2026-02-14 Session

**Detailed Trace with Unlimited Logging:**

Problematic address 0x81B96 (varies per run due to DOS memory allocation):
```
22:38:15.605 - READ 81B96 = 00 (1st read, uninitialized) ❌
22:38:15.610 - READ 81B96 = 00 (2nd read) ❌
22:38:15.689 - READ 81B96 = 00 (3rd read) ❌
22:38:16.331 - READ 81B96 = 00 (4th read) ❌
22:38:16.348 - READ 81B96 = 00 (5th read) ❌
22:38:16.437 - READ 81B96 = 00 (6th read) ❌
22:38:16.448 - READ 81B96 = 00 (7th read) ❌
22:38:16.454 - READ 81B96 = 00 (8th read) ❌
22:38:16.763 - WRITE 81B96 = F3 (correct value, too late!) ✓
22:38:16.879 - READ 81B96 = F3 (all subsequent reads correct)
```

**Key Findings:**

1. **Initialization completes "successfully"**: 12,185 writes to 0x7F000-0x83000 region
2. **Program continues running**: No crash, no immediate error - KEEN executes normally after initialization
3. **Error appears later**: "Error during code expansion!" only appears when decompressor runs and tries to use corrupted state tables
4. **Sequential processing**: KEEN processes EGAHEAD.CK1 linearly from start to finish
5. **This is how KEEN's code works**: It's not a control flow bug - the code intentionally reads before writing
6. **Final state is correct**: ram.bin shows 0x81B96 = 0xF3 after initialization completes

**State Table Corruption:**
- Address 0x80010 should contain 0x0B00
- Actually contains 0x06BE (verified in ram.bin hexdump)
- Caused by reading 0x00 from 0x81F73 during state table construction

## The Core Mystery

**KEEN's initialization code has a circular dependency:**
- Reads from 0x81B96 during early file processing (expects valid data)
- Writes to 0x81B96 during later file processing (provides the data)
- In our emulator: reads get 0x00 → wrong state tables → decompressor fails
- On real hardware: reads get ??? → correct state tables → game works

**The Question**: What values are in those "uninitialized" addresses on real hardware that make KEEN work?

**Possibilities:**
1. **CPU instruction bug**: Our emulator executes instructions in slightly different order than real hardware
2. **Cache behavior**: Real CPU caches might make writes visible to reads in unexpected ways
3. **Memory controller timing**: Real hardware memory access timing differs from our instantaneous reads/writes
4. **KEEN has a latent bug**: Works on real hardware by accident due to specific memory patterns
5. **DOS memory allocator difference**: Real DOS pre-fills allocated memory with specific patterns
6. **Interrupt timing**: Hardware interrupts fire at different times, affecting execution order

## Options for Further Investigation

### Option 1: DOSBox Comparison (Most Practical)
- Run KEEN in DOSBox with heavy logging enabled
- Compare instruction-by-instruction execution with our emulator
- Look for differences in: memory access patterns, register values, branch decisions
- **Effort**: Medium | **Likelihood of success**: High

### Option 2: CPU Instruction Review (Tedious but Thorough)
- Review implementations of all instructions KEEN uses during initialization
- Focus on: LOOP, REP MOVSB/STOSB, conditional jumps, segment register loads
- Look for subtle bugs in flag handling, off-by-one errors, or timing issues
- **Effort**: Very High | **Likelihood of success**: Medium

### Option 3: Test on Real Hardware (Definitive but Requires Hardware)
- Run KEEN on actual 286/386 PC
- Use debug tools to dump memory at 0x81B96 before first read
- See what actual values exist in "uninitialized" memory
- **Effort**: High (requires hardware) | **Likelihood of success**: Very High (would give definitive answer)

### Option 4: Segment Register Logging (Quick Test)
- Add logging for DS/ES/SS register values during initialization
- Check if segment calculations are producing wrong physical addresses
- Look for unexpected segment register modifications
- **Effort**: Low | **Likelihood of success**: Low (already verified physical addresses are correct)

### Option 5: Known KEEN Bugs Research (Quick Check)
- Search for known bugs, patches, or compatibility issues with KEEN1
- Check if there's a specific CPU type or flag that affects initialization
- Look for community fixes or workarounds
- **Effort**: Very Low | **Likelihood of success**: Low but worth checking

### Option 6: Accept the Bug and Work Around It (Pragmatic)
- Pre-initialize the 0x81xxx region with specific values that make KEEN work
- Extract correct values from successful DOSBox run
- Treat as a compatibility quirk rather than fixing root cause
- **Effort**: Low | **Likelihood of success**: High (but doesn't solve the underlying issue)

## 2026-02-25 Investigation: LZW Dictionary Self-Reference

### Approach

Investigated the decompression failure using a full instruction-level trace log (oxide86.log, ~3M lines). The decompression starts at log line ~814601 and the error appears at ~2916818.

### Error Mechanism

The error "Error during code expansion!" is triggered when the LZW dictionary grows to 4000 entries (0x0FA0). At line ~2915669:
```
0FD8:6BAB cmp ax, 0x0FA0    ; AX = current dictionary entry count
0FD8:6BAE jl 0x6bc2          ; NOT taken - 0x0FA0 is NOT < 0x0FA0
0FD8:6BB0 mov ax, 0x2853     ; error path - loads error string pointer
```

### Root Cause: Circular Dictionary Entry

**Dictionary entry at index 0x06BE contains 0x06BE** — it points to itself, creating an infinite chain walk loop.

The chain walk function at 0FD8:6B6E-6BC7 reads `es:[bx]` which resolves to linear address 0x8B840, always getting value 0x06BE. This causes `[bp+0x06]` to be 0x06BE > 0x00FF, so the `ja 0x6b6e` loop keeps iterating, incrementing the dictionary counter DI on each pass until it hits the 0x0FA0 limit.

### How the Self-Reference Was Created

Traced the exact write at log line ~1532172:
```
0FD8:6B0D mov es:[bx], ax   ; AX=06BE, writes to @8B84:0000 (linear 0x8B840)
```

The LZW algorithm at this point:
- **DI** (next dictionary entry to fill) = **0x06BE**
- **[bp-0x04]** (old_code / parent pointer) = **0x06BE**
- Result: entry 0x06BE → 0x06BE (self-reference)

In correct LZW, `old_code` should always be **less than** the current dictionary index (DI), because old_code was a valid code from a previous iteration.

### Why old_code Equals DI

Traced backwards to find when `[bp-0x02]` (new_code) was first set to 0x06BE (log line ~1531673):

1. **Huffman bit reader** extracts code **0x06BE** from the 32-bit accumulator DX:AX = 0xD7D6:7F56 via a 21-bit right shift (32 - 11 bit code width = 21). Verified: `0xD7D6 >> 5 = 0x06BE` ✓

2. At the `cmp [bp-0x02], di` check (line ~1531681): new_code 0x06BE was compared with DI. The `jb` (jump if below) was **NOT taken**, meaning 0x06BE >= DI. This is the **"code not in dictionary"** special case in LZW.

3. **DI at that point was 0x06BD** (verified via `shl bx, 1` → BX=0x0D78, so DI = 0x06BC, then `inc di` → 0x06BD).

4. So **new_code (0x06BE) = DI + 1**. In standard LZW, the ONLY valid "code not in dictionary" case is `new_code == DI` (exactly one ahead). Code `DI + 1` is **INVALID** — it means the bitstream is being misinterpreted.

5. After processing code 0x06BE via the special path, `old_code` is set to 0x06BE, DI increments to 0x06BE, and on the next iteration, the dictionary write creates entry 0x06BE → old_code 0x06BE → self-reference.

### Verified NOT the Cause

- **SHL/SHR/RCL/CBW instructions**: All verified correct by tracing actual values
- **NEG instruction**: Implementation verified correct (`wrapping_neg()`)
- **5-bit shift count masking** (286+ behavior on 8086): Shift counts in the decompression loop are all 0-24, within the 5-bit mask range; doesn't cause differences
- **E169 path** (32-bit shift left with count >= 16): Only hit twice in the entire decompression, both with CL=16 producing correct zero results
- **CWD instruction**: Always operates on AX=0x00-0xFF (byte values), so DX is always 0
- **32-bit shift right (E192)**: Verified code extraction produces correct value from accumulator

### Suspected: Bit Accumulator Off-by-One

The 32-bit bit accumulator (`[0x82F2..0x82F5]`, bit count `[0x82F6]`) accumulates 8 bits per input byte and extracts 9-12 bit LZW codes. If the accumulator gets even a **single bit wrong** at any point, all subsequent code extractions will be misaligned, eventually producing an invalid code.

The code 0x06BE was extracted correctly from the accumulator value — the question is whether the accumulator itself had the right data. This could be caused by:
1. **Wrong data read from disk** (INT 13h sector read returning incorrect bytes)
2. **Wrong data in the file buffer** (DOS file I/O corruption)
3. **A cumulative bit error** from earlier in the decompression that shifted the bit stream

### LZW Code Width Transitions

Commander Keen's LZW uses variable-width codes with "early change" transitions:
- Width increases when `DI == threshold` (threshold = `(1 << width) - 1`)
- Transitions observed: 9→10 bits at DI=0x01FF, 10→11 at DI=0x03FF, 11→12 at DI=0x07FF, 12→max at DI=0x0FFF
- Max code width is 12 bits (`[0x8326]` = 0x000C)
- **Could be a width timing mismatch** — if the compressor uses "late change" and our decompressor triggers "early change", a 1-bit misalignment would accumulate

### Key Architecture Detail

File I/O goes through **real DOS from the boot disk**, not the emulator's BIOS:
- INT 21h vector → DOS kernel at segment 035C → INT 13h → emulator's disk sector I/O
- This means the emulator's INT 21h file read handler is NOT involved
- Any file I/O bug would be in INT 13h (disk sector reads) or the real DOS's buffering

### Decompression Algorithm Summary

```
Commander Keen LZW Decompressor (0FD8:6A30-6C48):

OUTER LOOP (at 0x6A40):
  new_code = read_huffman_code()      ; calls 0x6BD4 (bit reader)
  if new_code == 0x0101: break        ; end of stream

  if new_code < DI:                   ; code IS in dictionary
    output = walk_chain(new_code)     ; calls 0x6B62
  else:                               ; code NOT in dictionary (== DI or > DI)
    output[0] = first_char            ; prepend first_char
    output[1..] = walk_chain(old_code); calls 0x6B62

  dict[DI].parent = old_code          ; 0x6B0D: word write to code table
  dict[DI].char = first_char          ; 0x6B24: byte write to char table
  DI++                                ; 0x6B27: inc di

  if DI == threshold:                 ; 0x6B28: width change check
    if width < max_width:
      width++
      threshold = (1 << width) - 1

  old_code = new_code

HUFFMAN BIT READER (0x6BD4):
  while bit_count <= 24:              ; accumulate bytes
    byte = input_buffer[ptr++]        ; read from far pointer
    accumulator |= (byte << (24 - bit_count))  ; 32-bit shift left
    bit_count += 8
  code = accumulator >> (32 - width)  ; 32-bit shift right to extract
  accumulator <<= width               ; consume bits
  bit_count -= width
  return code

Dictionary tables:
  Code table (word): base at [0x8266]:[0x8268] = 0x0004:0x8AAC
    Entry N at: base + N*2 (far pointer normalized via E23B)
  Char table (byte): base at [0xA6D6]:[0xA6D8] = 0x0004:0x8D20
    Entry N at: base + N (far pointer normalized via E23B)
```

### Next Steps

1. **Re-run with trace logging** to reproduce the log (was overwritten)
2. **Verify INT 13h sector reads**: Compare raw sector data from emulator vs known-good disk image
3. **Binary search the bit accumulator**: Sample accumulator state at the midpoint (~line 1.8M) to see if errors have already accumulated
4. **Check code width transition timing**: Compare with known Commander Keen LZW implementations (id Software's IGRAB/LZSS) to confirm early vs late change
5. **DOSBox comparison** at the width transition boundary: extract the exact accumulator value at DI=0x01FF and compare

## Recommendation

Start with **Option 1 (DOSBox comparison)** - it's the most practical way to see the actual difference in execution. If that reveals the issue, great. If not, move to **Option 3 (real hardware testing)** if available, otherwise **Option 6 (workaround)** while keeping the issue documented for future investigation.

The 2026-02-25 investigation strongly suggests the root cause is either:
- **A bit-level misalignment in the Huffman accumulator** (caused by wrong input data or a subtle shift instruction bug under specific conditions not yet observed)
- **A code width transition timing mismatch** between compressor and decompressor

The most efficient next step is to **compare the bit accumulator state at a few key points** (start, midpoint, near the error) between the emulator and DOSBox to pinpoint exactly where divergence begins.
