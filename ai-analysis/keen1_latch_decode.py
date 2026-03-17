#!/usr/bin/env python3
"""
Commander Keen 1 — EGALATCH.CK1 decompressor.

Reverse-engineered from oxide86.log by tracing the game's decoder in memory.

FILE FORMAT
-----------
Offset  Size  Description
     0     4  total_uncompressed_size (uint32 LE)   e.g. 119680
     4     2  type                   (uint16 LE)    = 12 → code_bits=9, max_dict=5021
     6     4  initial bit-buffer seed (4 bytes, MSB-first)
    10     …  LZW bit stream (MSB-first, variable-width codes)

LZW PARAMETERS (type = 12)
---------------------------
  initial code width : 9 bits
  code 0x000–0x0FF   : literals (single byte)
  code 0x100         : CLEAR — reset dictionary, code_bits back to 9
  code 0x101         : END   — stop
  code 0x102–0x139D  : dictionary entries (max 5021 total codes)
  code width grows   : when next_code reaches 512 → 10 bits, 1024 → 11, 2048 → 12, 4096 → 13

BIT BUFFER (matched to 102A:6BDB–6C43 in keen1.exe)
-----------------------------------------------------
  32-bit MSB-first accumulator stored as [hi16 : lo16].
  Fill condition : while valid_bits <= 24, read one byte, shift it into the
                   buffer at bit position (24 − valid_bits), add 8 to count.
  Code extraction: code = buffer >> (32 − code_bits);
                   then left-shift buffer by code_bits; subtract from count.

BUFFER OVERFLOW (102A:6BAB)
----------------------------
  The game stores each code's expanded string in a 4000-byte temp buffer.
  If any single dictionary chain is longer than 4000 bytes the game prints
  "Error during code expansion!" and aborts.  With max_dict=5021 the longest
  possible chain is 5021 − 258 = 4763 > 4000, so overflow is guaranteed once
  the dictionary is sufficiently built up.

ROOT CAUSE
----------
  The average decompressed chunk is ~9973 bytes (119680 / 12), which exceeds
  the 4000-byte buffer by ~2.5×.  The buffer was sized for CGA (80×50 = 4000
  bytes) and was never updated when the engine was ported to EGA.
"""

import struct
import sys
import os


# ---------------------------------------------------------------------------
# Parameters for type=12 (the only type present in EGALATCH.CK1)
# ---------------------------------------------------------------------------
INITIAL_CODE_BITS = 9
MAX_CODE_BITS     = 12     # [0x8326]=12; code_bits capped here
MAX_DICT_CODES    = 1 << MAX_CODE_BITS   # 4096 total code slots (0x000–0xFFF)
#   Initial threshold = (1<<9)-1 = 511; grows to 1023, 2047, 4095.
#   Dict entries added only when next_code ≤ threshold (ja check at 6AEF).
#   code_bits increases when next_code (after increment) == threshold (log: inc [0x96fa] when DI=0x01FF).
#   Max entry code = 4095; max chain depth = 4095-256 = 3839 < 4000.
CLEAR_CODE        = 0x100
END_CODE          = 0x101
DICT_START        = 0x102
GAME_BUFFER_LIMIT = 4000   # 0x0FA0 at 102A:6BAB — per single code expansion


# ---------------------------------------------------------------------------
# Bit reader — mirrors the game's 32-bit MSB-first accumulator
# ---------------------------------------------------------------------------
class BitReader:
    def __init__(self, data: bytes, offset: int):
        self.data  = data
        self.pos   = offset
        self.buf   = 0       # 32-bit MSB-first accumulator
        self.bits  = 0       # valid bit count

    def _fill(self) -> None:
        """Fill until valid_bits > 24 (mirrors 102A:6BDB–6C0E)."""
        while self.bits <= 24 and self.pos < len(self.data):
            byte = self.data[self.pos]
            self.pos += 1
            shift = 24 - self.bits
            self.buf = (self.buf | (byte << shift)) & 0xFFFFFFFF
            self.bits += 8

    def read_code(self, n: int) -> int:
        """Extract the next n-bit code (mirrors 102A:6C15–6C43)."""
        self._fill()
        code = (self.buf >> (32 - n)) & ((1 << n) - 1)
        self.buf  = (self.buf << n) & 0xFFFFFFFF
        self.bits -= n
        return code


# ---------------------------------------------------------------------------
# LZW decoder
# ---------------------------------------------------------------------------
def lzw_decode(data: bytes, offset: int, expected_size: int) -> tuple[bytes, int, int]:
    """
    Decode one LZW stream starting at `offset`.
    Returns (output_bytes, bytes_consumed_from_stream, max_chain_depth_seen).
    Raises OverflowError if any chain exceeds GAME_BUFFER_LIMIT (mirroring the
    game's actual abort).
    """
    reader     = BitReader(data, offset)
    code_bits  = INITIAL_CODE_BITS
    next_code  = DICT_START

    # Dictionary: code → bytes string
    # Pre-populate literals 0–255
    dictionary: dict[int, bytes] = {i: bytes([i]) for i in range(256)}

    output       = bytearray()
    prev_str     = None   # expansion of the previous code
    max_chain    = 0
    threshold    = (1 << code_bits) - 1   # 511; grows to 1023, 2047, 4095

    while True:
        code = reader.read_code(code_bits)

        if code == END_CODE:
            break

        if code == CLEAR_CODE:
            # Reset decoder state
            dictionary = {i: bytes([i]) for i in range(256)}
            next_code  = DICT_START
            code_bits  = INITIAL_CODE_BITS
            threshold  = (1 << code_bits) - 1
            prev_str   = None
            continue

        # Expand code to a byte string
        if code < next_code:
            curr_str = dictionary[code]
        elif code == next_code:
            # Standard LZW: not-yet-defined code — self-referential entry
            assert prev_str is not None, f"not-yet-defined code {code:#x} with no prev"
            curr_str = prev_str + prev_str[:1]
        else:
            raise ValueError(f"invalid LZW code {code:#x}, next_code={next_code:#x}")

        # Track max chain depth (mirrors DI check at 102A:6BAB)
        chain_len = len(curr_str)
        if chain_len > max_chain:
            max_chain = chain_len
        if chain_len > GAME_BUFFER_LIMIT:
            raise OverflowError(
                f"chain depth {chain_len} > {GAME_BUFFER_LIMIT} at code {code:#x} "
                f"(102A:6BAB overflow — game would print 'Error during code expansion!')"
            )

        output.extend(curr_str)

        # Add new dictionary entry
        # ja check at 102A:6AEF: skip if next_code > threshold
        if prev_str is not None and next_code <= threshold:
            dictionary[next_code] = prev_str + curr_str[:1]
            next_code += 1
            # Grow code width when next_code reaches threshold
            # (game: cmp DI,[0x8348]; jne skip; inc [0x96fa] at 102A:6B28/6B37)
            if next_code == threshold and code_bits < MAX_CODE_BITS:
                code_bits += 1
                threshold = (1 << code_bits) - 1

        prev_str = curr_str

        if len(output) >= expected_size:
            break

    bytes_consumed = reader.pos - offset
    return bytes(output), bytes_consumed, max_chain


# ---------------------------------------------------------------------------
# Main report
# ---------------------------------------------------------------------------
def report(path: str) -> None:
    with open(path, 'rb') as f:
        data = f.read()

    total_size  = struct.unpack_from('<I', data, 0)[0]
    file_type   = struct.unpack_from('<H', data, 4)[0]
    seed_bytes  = data[6:10]

    print("=" * 64)
    print("EGALATCH.CK1 — LZW decompression")
    print("=" * 64)
    print(f"  File size (compressed) : {len(data):,} bytes")
    print(f"  Total uncompressed     : {total_size:,} bytes")
    print(f"  Type field             : {file_type} → code_bits=9, max_dict={MAX_DICT_CODES}")
    print(f"  Seed bytes (6–9)       : {seed_bytes.hex(' ')}")
    print()

    # Verify seed: load 4 bytes into 32-bit buffer, extract first 9-bit code
    buf = 0
    for i, b in enumerate(seed_bytes):
        buf |= b << (24 - i * 8)
    first_code = buf >> (32 - INITIAL_CODE_BITS)
    print(f"  Initial buffer         : 0x{buf:08X}")
    print(f"  First code (from seed) : 0x{first_code:03X} = {first_code}"
          f"  ({'literal 0x%02X' % first_code if first_code < 256 else 'dict/special'})")
    print()

    # Attempt full decode
    print("Decoding...")
    try:
        output, consumed, max_chain = lzw_decode(data, 6, total_size)
        print(f"  Decoded {len(output):,} bytes  (expected {total_size:,})")
        print(f"  Consumed {consumed:,} compressed bytes  (file has {len(data)-6:,} after header)")
        print(f"  Max chain depth seen   : {max_chain}  (limit: {GAME_BUFFER_LIMIT})")
        print()

        if len(output) >= total_size:
            print("  RESULT: Full decompression successful.")
        else:
            print(f"  RESULT: Short — only {len(output):,} of {total_size:,} bytes decoded.")

        # Show first 32 bytes as a sanity check
        print()
        print("  First 32 bytes of output:")
        print("  ", output[:32].hex(' '))

    except OverflowError as e:
        print(f"\n  *** OVERFLOW DETECTED ***")
        print(f"  {e}")
        print()
        print("  This matches the 'Error during code expansion!' crash in keen1.exe.")
        print(f"  Buffer limit {GAME_BUFFER_LIMIT} bytes = 80×50 CGA screen.")
        print(f"  EGALATCH.CK1 contains EGA data with avg chunk ~{total_size//12:,} bytes.")
        print(f"  Ratio: {total_size / 12 / GAME_BUFFER_LIMIT:.1f}× buffer size.")

    except Exception as e:
        print(f"\n  ERROR: {e}")
        import traceback; traceback.print_exc()


if __name__ == '__main__':
    base  = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                         '..', 'target', 'keen1')
    latch = os.path.join(base, 'EGALATCH.CK1')
    if len(sys.argv) > 1:
        latch = sys.argv[1]

    if not os.path.exists(latch):
        print(f"File not found: {latch}", file=sys.stderr)
        sys.exit(1)

    report(latch)
