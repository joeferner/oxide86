# CGA Composite Mode Implementation Plan

## Background

CGA hardware can output an NTSC composite video signal in addition to RGB. In 640×200 2-color mode (mode 06h), this creates a well-known "composite artifact color" effect: because the NTSC color subcarrier runs at exactly 4× the CGA pixel clock, groups of 4 adjacent pixels encode a chrominance phase, producing colors beyond black and white.

The `trans.asm` test draws a trans pride flag by exploiting this:
- `0x3333` (bits: `00110011`) → nibble 3 per 4-pixel group → **light blue**
- `0xEEEE` (bits: `11101110`) → nibble 14 per 4-pixel group → **light pink**
- `0xFFFF` (all ones) → nibble 15 → **white**

Currently, `render_mode_06h_640x200x2()` renders mode 06h as pure black/white, ignoring the colorburst. The test reference `trans.png` (640×350, likely a placeholder) needs regeneration after implementation.

---

## CGA Composite Color Theory

### Colorburst enable: port 0x3D8 bit 3

Writing `0x1A` (`00011010b`) to port 0x3D8:
- bit 1 = 1 → graphics mode
- bit 3 = 1 → **colorburst enable** (composite output active)
- bit 4 = 1 → 640×200 high-res mode → Mode 06h

Bit 3 is currently stored in `cga_mode_ctrl` but not forwarded to `VideoBuffer`.

### Color phase: port 0x3D9 bit 5

Writing `0x2F` (`00101111b`) to port 0x3D9:
- bits 3:0 = 0xF → background color (irrelevant in mode 06h)
- bit 5 = 1 → **alternate composite phase** (shifts palette by 90°, i.e. 1 pixel)

This bit already updates `cga_palette` in `VideoBuffer` via `set_cga_color_select()`.

### Artifact color decoding

In the composite signal each 4-pixel group forms a "color cell":
- Collect 4 consecutive 1-bit pixels into a nibble (MSB first = leftmost pixel)
- The nibble (0–15) indexes into the composite color table
- `cga_palette` (bit 5 of 0x3D9) selects between two phase-shifted tables

The two palettes cover all 16 nibble patterns. Phase 1 (bit 5 set) is equivalent to the nibble being read starting from an odd column (1-pixel phase shift).

#### Phase 0 palette (bit 5 of 0x3D9 = 0)

| Nibble | Pattern | Color (approx.) | R,G,B |
|--------|---------|-----------------|-------|
| 0 | 0000 | Black | 0, 0, 0 |
| 1 | 0001 | Dark blue | 0, 0, 170 |
| 2 | 0010 | Dark green | 0, 170, 0 |
| 3 | 0011 | Cyan/blue | 0, 170, 170 |
| 4 | 0100 | Dark red | 170, 0, 0 |
| 5 | 0101 | Magenta | 170, 0, 170 |
| 6 | 0110 | Brown | 170, 85, 0 |
| 7 | 0111 | Light gray | 170, 170, 170 |
| 8 | 1000 | Dark gray | 85, 85, 85 |
| 9 | 1001 | Light blue | 85, 85, 255 |
| 10 | 1010 | Light green | 85, 255, 85 |
| 11 | 1011 | Light cyan | 85, 255, 255 |
| 12 | 1100 | Light red | 255, 85, 85 |
| 13 | 1101 | Light magenta/pink | 255, 85, 255 |
| 14 | 1110 | Yellow | 255, 255, 85 |
| 15 | 1111 | White | 255, 255, 255 |

#### Phase 1 palette (bit 5 of 0x3D9 = 1)

Phase 1 shifts by one pixel, which in practice re-maps the nibbles. The effect is that the high/low nibble halves swap roles. For the trans flag (`0x3333` and `0xEEEE`) with **phase 1**:
- `0x3333` → nibble 3 at phase 1 → **light blue** (matches the flag)
- `0xEEEE` → nibble 14 at phase 1 → **pink/light magenta** (matches the flag)

The phase 1 palette should be defined by reading the phase-0 palette starting one pixel later, effectively rotating nibble bits: `nibble_phase1 = ((nibble << 1) | (nibble >> 3)) & 0xF`.

> **Note:** Exact RGB values should be tuned against real CGA hardware captures or DOSBox composite output. The values above are the standard 6-bit EGA approximations and should be close enough for passing the snapshot test.

---

## Implementation Steps

### Step 1 — Track colorburst enable in VideoBuffer

**File:** [core/src/video/video_buffer.rs](core/src/video/video_buffer.rs)

Add a `cga_composite: bool` field alongside `cga_bg`/`cga_intensity`/`cga_palette`:

```rust
/// Composite (colorburst) output enabled (bit 3 of port 0x3D8).
/// When true and mode is M06Cga640x200x2, renders artifact colors instead of B&W.
cga_composite: bool,
```

Add accessor and setter:

```rust
pub(crate) fn set_cga_composite(&mut self, enabled: bool) {
    self.cga_composite = enabled;
    self.dirty = true;
}
```

Initialize to `false` in `new()` and `reset()`.

**File:** [core/src/video/video_card.rs](core/src/video/video_card.rs)

In the `CGA_MODE_CTRL_ADDR` write handler (~line 841), extract bit 3 and forward it:

```rust
CGA_MODE_CTRL_ADDR => {
    self.cga_mode_ctrl = val;
    let composite = (val & 0x08) != 0;  // bit 3 = colorburst enable
    let mode = /* ... existing mode derivation ... */;
    log::info!("CGA mode control 0x3D8 = 0x{:02X} → mode {}, composite={}", val, mode, composite);
    let mut buffer = self.buffer.write().unwrap();
    buffer.set_mode(mode);
    buffer.set_cga_composite(composite);
    true
}
```

### Step 2 — Add composite color palettes

**File:** [core/src/video/video_buffer.rs](core/src/video/video_buffer.rs)

Add two const arrays at module level (or as associated consts):

```rust
/// CGA composite artifact colors, phase 0 (bit 5 of 0x3D9 = 0).
/// Index = 4-bit pixel nibble (MSB = leftmost pixel). RGB bytes.
const CGA_COMPOSITE_PHASE0: [[u8; 3]; 16] = [
    [0,   0,   0  ], // 0  black
    [0,   0,   170], // 1  dark blue
    [0,   170, 0  ], // 2  dark green
    [0,   170, 170], // 3  cyan/blue
    [170, 0,   0  ], // 4  dark red
    [170, 0,   170], // 5  magenta
    [170, 85,  0  ], // 6  brown
    [170, 170, 170], // 7  light gray
    [85,  85,  85 ], // 8  dark gray
    [85,  85,  255], // 9  light blue
    [85,  255, 85 ], // 10 light green
    [85,  255, 255], // 11 light cyan
    [255, 85,  85 ], // 12 light red
    [255, 85,  255], // 13 light magenta / pink
    [255, 255, 85 ], // 14 yellow
    [255, 255, 255], // 15 white
];

/// CGA composite artifact colors, phase 1 (bit 5 of 0x3D9 = 1).
/// Equivalent to phase 0 but with each nibble rotated left by 1 bit.
const CGA_COMPOSITE_PHASE1: [[u8; 3]; 16] = {
    let mut table = [[0u8; 3]; 16];
    let mut i = 0;
    while i < 16 {
        let rotated = ((i << 1) | (i >> 3)) & 0xF;
        table[i] = CGA_COMPOSITE_PHASE0[rotated];
        i += 1;
    }
    table
};
```

> These are compile-time constants. Adjust RGB values after generating the reference PNG against real hardware behavior.

### Step 3 — Add composite rendering path

**File:** [core/src/video/video_buffer.rs](core/src/video/video_buffer.rs)

In `render_into()`, update the mode 06h arm:

```rust
Mode::M06Cga640x200x2 => {
    if self.cga_composite {
        self.render_mode_06h_composite(buf)
    } else {
        self.render_mode_06h_640x200x2(buf)
    }
}
```

### Step 4 — Implement `render_mode_06h_composite()`

**File:** [core/src/video/video_buffer.rs](core/src/video/video_buffer.rs)

Output resolution: **640×400** (same as non-composite mode 06h; each composite color block is 4 pixels wide × 2 scanlines tall to keep the allocation consistent with `mode.resolution()`).

```rust
/// Render CGA 640x200 mode 06h in composite (colorburst) mode.
///
/// CGA VRAM interleaved layout: even scanlines at 0x0000, odd at 0x2000.
/// Every 4 consecutive pixel-bits form a nibble (MSB = leftmost).
/// The nibble indexes into a composite artifact color palette (16 colors).
/// The palette phase is controlled by `cga_palette` (bit 5 of port 0x3D9):
///   false = phase 0, true = phase 1 (90° NTSC phase shift, 1-pixel offset).
/// Output is doubled vertically (640×400) for CRT aspect ratio.
fn render_mode_06h_composite(&self, buf: &mut [u8]) {
    const SRC_WIDTH: usize = 640;
    const OUT_WIDTH: usize = 640;
    const SRC_HEIGHT: usize = 200;

    let palette = if self.cga_palette {
        &CGA_COMPOSITE_PHASE1
    } else {
        &CGA_COMPOSITE_PHASE0
    };

    for y in 0..SRC_HEIGHT {
        let bank_offset = if y % 2 == 1 { 0x2000 } else { 0 };
        let row_base = bank_offset + (y / 2) * 80;

        // Collect the 4-pixel nibble for each group and emit 4 pixels of that color
        for group in 0..(SRC_WIDTH / 4) {
            let mut nibble: usize = 0;
            for bit in 0..4 {
                let x = group * 4 + bit;
                let byte_val = self.vram[row_base + x / 8];
                let bit_val = (byte_val >> (7 - (x % 8))) & 1;
                nibble = (nibble << 1) | bit_val as usize;
            }
            let rgb = palette[nibble];
            for bit in 0..4 {
                let x = group * 4 + bit;
                // Double each scanline for CRT aspect ratio
                for dy in 0..2 {
                    let offset = ((y * 2 + dy) * OUT_WIDTH + x) * 4;
                    buf[offset]     = rgb[0];
                    buf[offset + 1] = rgb[1];
                    buf[offset + 2] = rgb[2];
                    buf[offset + 3] = 0xFF;
                }
            }
        }
    }
}
```

### Step 5 — Regenerate the reference PNG

The existing `trans.png` (640×350) appears to be a placeholder and will not match the new 640×400 composite output. After implementing Steps 1–4:

1. Run the test once to produce `trans_actual.png`
2. Visually verify the output shows the correct trans flag colors (blue / pink / white / pink / blue horizontal stripes)
3. Copy `trans_actual.png` → `trans.png` to lock in the reference

```bash
cargo test --all cga_composite_trans
cp core/src/test_data/video/cga_composite/trans_actual.png \
   core/src/test_data/video/cga_composite/trans.png
```

---

## Files to Modify

| File | Change |
|------|--------|
| [core/src/video/video_buffer.rs](core/src/video/video_buffer.rs) | Add `cga_composite` field; add palette constants; add composite render path and method |
| [core/src/video/video_card.rs](core/src/video/video_card.rs) | Extract bit 3 from 0x3D8 writes and forward via `set_cga_composite()` |
| [core/src/test_data/video/cga_composite/trans.png](core/src/test_data/video/cga_composite/trans.png) | Replace with correct composite output (generated post-implementation) |

No new files needed. The `reset()` path in `VideoBuffer` already covers initialization.

---

## Edge Cases and Notes

- **Mode 04h (320×200x4):** CGA composite is only active in mode 06h. Mode 04h already produces colors via the EGA palette and does not need composite treatment.
- **Non-CGA card types:** EGA/VGA do not have a colorburst output. The `cga_composite` flag should only be checked when the card type is `VideoCardType::CGA`. This is naturally satisfied because `CGA_MODE_CTRL_ADDR` writes only reach mode derivation for CGA/EGA/VGA cards, but could be guarded explicitly if needed.
- **Port 0x3D9 bit 5 in mode 04h:** Already interpreted as `cga_palette` for palette selection — that existing behavior is correct and unchanged.
- **Color accuracy:** The RGB values in the palette constants are based on the standard 16 EGA colors (2/3 intensity steps). Real CGA composite colors depend on analog NTSC decoding and vary by monitor. If visual accuracy is important, values can be tuned against captures from DOSBox's CGA composite mode or MAME's CGA emulation.
- **The `cga_composite` flag and `dirty` flag:** Setting `cga_composite` marks the buffer dirty, so a re-render is triggered automatically.
