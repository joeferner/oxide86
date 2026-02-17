# VGA Mode 13h Implementation Plan

## Overview

Implement VGA mode 13h (320x200, 256 colors) - the most popular DOS graphics mode.

## Mode 13h Specifications

- **Resolution**: 320x200 pixels
- **Colors**: 256 simultaneous colors from VGA DAC palette
- **Memory**: A000:0000 - A000:FFFF (same as EGA)
- **Layout**: **Linear framebuffer** (NOT planar like EGA mode 0x0D)
  - 64,000 bytes total (320 × 200)
  - One byte per pixel: direct color index 0-255
  - Offset calculation: `y * 320 + x`
- **Palette**: VGA DAC (256 entries × 3 bytes RGB, 6 bits per channel)

## Key Differences from EGA Mode 0x0D

| Feature | EGA Mode 0x0D | VGA Mode 13h |
|---------|---------------|--------------|
| Memory layout | Planar (4 planes × 8000 bytes) | Linear (64000 bytes) |
| Colors | 16 (4 bits) | 256 (8 bits) |
| Pixel access | 4 planes, bit composition | Direct byte access |
| Complexity | Complex (map mask, read plane) | Simple (one byte = one pixel) |

**Critical insight**: Mode 13h is MUCH simpler than mode 0x0D because it's linear, not planar.

## Architecture After commit 334d73a ("fix ega")

This commit refactored the video subsystem significantly. The plan must follow the
new patterns, NOT the old patterns (which used separate `CgaBuffer`/`EgaBuffer` structs):

**New pattern (current codebase):**
- `CgaBuffer` struct is **removed** — CGA pixels computed on-demand from `vram` via `get_cga_pixels()`
- `EgaBuffer` struct and `ega.rs` file are **removed** — EGA planes stored directly in `vram[plane*8000..]`
- `Video.vram` is the single source of truth for all video memory
- `bus.rs` routes A000 writes via `video.write_byte_ega()` → `vram` directly (no drain Vec)
- Rendering calls `bus.get_cga_pixels()` / `bus.get_ega_pixels()` which return `Vec<u8>` on demand

**Mode 13h must follow the same pattern:**
- Linear pixels stored directly in `vram[0..64000]`
- `Video.write_byte_vga(offset, value)` writes `vram[offset] = value`
- `Video.get_vga_pixels()` returns `Vec<u8>` by copying `vram[0..64000]` (or a slice reference)
- `bus.rs` routes A000 writes in mode 13h to `write_byte_vga()`
- `computer.rs` calls `bus.get_vga_pixels()` for rendering

**Critical constraint: vram must be enlarged**
- Current `CGA_MEMORY_SIZE = 32768` (32KB) — enough for CGA (16KB) and EGA (4×8000 = 32000 bytes)
- Mode 13h requires **64000 bytes** — exceeds current 32KB vram
- Solution: introduce `VIDEO_MEMORY_SIZE = 65536` (64KB) and use it for `vram`
- Rename constant (and update all references) or add a new one

## Implementation Plan

### Phase 1: Expand vram and add VideoMode variant

**File: `core/src/video/mod.rs`**

1. Add `VIDEO_MEMORY_SIZE` constant and update `vram` field:
   ```rust
   pub const VIDEO_MEMORY_SIZE: usize = 65536; // 64KB — enough for mode 13h (64000 bytes)
   // Keep CGA_MEMORY_SIZE for the address range constant; rename the vram size
   vram: Box<[u8; VIDEO_MEMORY_SIZE]>,
   ```

2. Add new enum variant to `VideoMode`:
   ```rust
   /// VGA 320x200, 256 colors (mode 0x13)
   Graphics320x200x256,
   ```

3. Update `Video::set_mode()` to handle mode 0x13:
   ```rust
   0x13 => VideoMode::Graphics320x200x256,
   ```

4. Add `write_byte_vga()` (linear, no plane logic):
   ```rust
   pub fn write_byte_vga(&mut self, offset: usize, value: u8) {
       if offset < 64000 {
           self.vram[offset] = value;
           self.dirty = true;
       }
   }
   ```

5. Add `read_byte_vga()`:
   ```rust
   pub fn read_byte_vga(&self, offset: usize) -> u8 {
       if offset < 64000 { self.vram[offset] } else { 0 }
   }
   ```

6. Add `get_vga_pixels()` (returns linear pixel data):
   ```rust
   pub fn get_vga_pixels(&self) -> Vec<u8> {
       self.vram[0..64000].to_vec()
   }
   ```

7. Update mode helpers to include `Graphics320x200x256`:
   - `get_cols()`: return 40 (40 text columns @ 8px/char)
   - `get_rows()`: return 25
   - `is_graphics_mode()` / `is_text_mode()` checks

8. Add scrolling support directly in `vram` (same pattern as Graphics320x200):
   In `scroll_up_window()` and `scroll_down_window()`, add a `Graphics320x200x256` arm:
   ```rust
   VideoMode::Graphics320x200x256 => {
       // 1 byte per pixel, 320 bytes per scan line, 8 scan lines per text row
       let scroll_lines = if lines == 0 { bottom - top + 1 } else { lines };
       for row in top..=bottom {
           let src_row = row + scroll_lines;
           for py in 0..8usize {
               let dst_y = row * 8 + py;
               let dst_base = dst_y * 320;
               if src_row <= bottom {
                   let src_base = (src_row * 8 + py) * 320;
                   let cx_start = left * 8;
                   let cx_end = (right + 1) * 8;
                   self.vram.copy_within(src_base + cx_start..src_base + cx_end, dst_base + cx_start);
               } else {
                   self.vram[dst_base + left*8..dst_base + (right+1)*8].fill(0);
               }
           }
       }
       self.dirty = true;
   }
   ```

9. Update `rebuild_cache()` to include `Graphics320x200x256` (no-op, vram is source of truth).

### Phase 2: Memory routing in bus.rs

**File: `core/src/bus.rs`**

1. In `read_u8()`, add mode 13h routing for A000 reads:
   ```rust
   if (EGA_MEMORY_START..=EGA_MEMORY_END).contains(&address) {
       let offset = address - EGA_MEMORY_START;
       return match self.video.get_mode_type() {
           crate::video::VideoMode::Graphics320x200x256 => self.video.read_byte_vga(offset),
           _ => self.video.read_byte_ega(offset),
       };
   }
   ```

2. In `write_u8()`, add mode 13h routing for A000 writes:
   ```rust
   if (EGA_MEMORY_START..=EGA_MEMORY_END).contains(&address) {
       let offset = address - EGA_MEMORY_START;
       match self.video.get_mode_type() {
           crate::video::VideoMode::Graphics320x200x256 => self.video.write_byte_vga(offset, value),
           _ => self.video.write_byte_ega(offset, value),
       }
       return;
   }
   ```

3. Add `get_vga_pixels()` (follows same pattern as `get_cga_pixels()` and `get_ega_pixels()`):
   ```rust
   /// Get VGA pixel data (320×200 256-color linear framebuffer)
   pub fn get_vga_pixels(&self) -> Vec<u8> {
       self.video.get_vga_pixels()
   }
   ```

   No `get_vga_buffer()` accessor needed — consistent with new design where buffers are not exposed.

### Phase 3: INT 10h Video BIOS

**File: `core/src/cpu/bios/int10.rs`**

1. Add mode 0x13 to supported modes list:
   ```rust
   0x00..=0x07 | 0x0D | 0x13 => {
       bus.video_mut().set_composite_mode(false);
       bus.video_mut().set_mode(mode, false);
       // ... BDA updates ...
   }
   ```

2. **AH=0Ch (Write Pixel)** — add arm for `Graphics320x200x256`:
   ```rust
   VideoMode::Graphics320x200x256 => {
       if col < 320 && row < 200 {
           bus.video_mut().write_byte_vga(row * 320 + col, color as u8);
       }
   }
   ```

3. **AH=0Dh (Read Pixel)** — add arm:
   ```rust
   VideoMode::Graphics320x200x256 => {
       if col < 320 && row < 200 {
           bus.video().read_byte_vga(row * 320 + col)
       } else {
           0
       }
   }
   ```

4. **Text drawing in graphics mode** (`draw_char_graphics()`):
   - Add `Graphics320x200x256` case alongside existing CGA/EGA cases
   - Same 320×200 coordinate system, 8×8 font
   - Use VGA DAC palette for foreground color

5. **Scrolling (AH=06h/07h)**: handled by `video.scroll_up_window()` / `scroll_down_window()` — just ensure the new vram arm is added (Phase 1).

### Phase 4: Rendering Pipeline

**File: `core/src/video/mod.rs` (VideoController trait)**

1. Add new trait method with default no-op:
   ```rust
   /// Update VGA graphics display (320x200, 256 colors, mode 0x13)
   /// pixel_data: linear pixel array (320*200 bytes), each byte is a 0-255 color index
   fn update_graphics_320x200x256(&mut self, pixel_data: &[u8]) {
       let _ = pixel_data;
       log::warn!("Graphics mode 320x200x256 (VGA mode 13h) not implemented for this platform");
   }
   ```

**File: `core/src/computer.rs`**

2. Add arm to `update_video()` match:
   ```rust
   crate::video::VideoMode::Graphics320x200x256 => {
       let pixels = self.bus.get_vga_pixels();
       self.video_controller.update_graphics_320x200x256(&pixels);
   }
   ```

3. Add same arm to `force_redraw()`.

**File: `native-gui/src/gui_video.rs`**

4. Implement rendering for native GUI:
   ```rust
   fn update_graphics_320x200x256(&mut self, pixel_data: &[u8]) {
       // Store for render loop
       self.graphics_data_256 = Some(pixel_data.to_vec());
       self.has_pending_updates = true;
   }
   ```

5. Add render method (scale 320×200 → display size, 2×2 typical):
   ```rust
   fn render_graphics_320x200x256(&mut self, pixels: &[u8], window: &Window) -> Result<()> {
       // For each pixel: color_index = pixels[y*320 + x]
       // RGB = vga_dac_6bit_to_8bit(self.vga_dac_palette[color_index])
       // Scale 320x200 → 640x400 (2x2 pixels each)
   }
   ```

**File: `wasm/src/web_video.rs`**

6. Implement for WASM using HTML5 Canvas:
   ```rust
   fn update_graphics_320x200x256(&mut self, pixel_data: &[u8]) {
       if let Err(e) = self.render_graphics_320x200x256(pixel_data) {
           log::error!("Failed to render 320x200x256 VGA graphics: {:?}", e);
       }
   }
   ```

### Phase 5: I/O Port Handling

No new I/O ports needed. Mode 13h uses standard VGA registers already implemented:
- VGA DAC (0x3C7/0x3C8/0x3C9): Palette read/write (already implemented, 256 entries)
- Sequencer Map Mask (0x3C4/0x3C5) not critical for linear mode (not planar)

### Phase 6: Testing & Validation

**Test Programs (create in `test-programs/video/`)**:

1. **mode13h-test.asm** - Basic mode test:
   ```asm
   ; Set mode 13h
   mov ax, 0013h
   int 10h

   ; Draw gradient bars (each color index = position mod 256)
   mov ax, 0A000h
   mov es, ax
   xor di, di
   mov cx, 320*200
   draw_loop:
       mov al, cl  ; color index = position
       stosb
       loop draw_loop

   ; Wait for key
   xor ax, ax
   int 16h

   ; Back to text mode
   mov ax, 0003h
   int 10h
   ret
   ```

2. **mode13h-palette.asm** - Test VGA DAC custom palette and INT 10h AH=0Ch/0Dh

3. **mode13h-text.asm** - Test text drawing in mode 13h (INT 10h AH=09h/0Eh)

**Real-world test programs**:
- **DOOM**: Uses mode 13h
- **Commander Keen 4-6**: Use mode 13h
- **Duke Nukem**: Uses mode 13h

**Validation checklist**:
- [ ] Mode 13h sets correctly via INT 10h AH=00h
- [ ] Linear memory writes to A000:0000 update framebuffer
- [ ] Pixel read/write (INT 10h AH=0Ch/0Dh) works
- [ ] Text drawing in graphics mode works
- [ ] Scrolling (INT 10h AH=06h/07h) works
- [ ] VGA DAC palette can be modified via INT 10h AH=10h
- [ ] Rendering displays correct colors via VGA DAC
- [ ] Mode switch back to text mode works
- [ ] No memory leaks or crashes

### Phase 7: Documentation

1. Update `CLAUDE.md` video section with mode 13h info
2. Update `test-programs/README.md` with new test programs
3. Update `MEMORY.md` if any critical bugs/patterns discovered

## Critical Implementation Notes

### Follow commit 334d73a Patterns

Do NOT create `VgaBuffer` struct. All pixel data lives in `Video.vram`:
- Mode 13h: `vram[offset]` where `offset = y * 320 + x` (linear, 0-63999)
- EGA mode 0x0D: `vram[plane * 8000 + offset]`
- CGA/Text: `vram[interlaced_offset]`

### vram Size Must Be Enlarged

- Current: `CGA_MEMORY_SIZE = 32768` (32KB)
- Required for mode 13h: 64000 bytes
- Recommended: introduce `VIDEO_MEMORY_SIZE = 65536` (64KB) and update the `vram` field
- All uses of `CGA_MEMORY_SIZE` as a vram size (not an address range) must be updated

### Bus Routing by Mode Type

Both EGA (0x0D) and VGA (0x13) use the A000 segment. Route based on current mode:
```rust
match self.video.get_mode_type() {
    Graphics320x200x256 => // VGA linear path
    _ => // EGA planar path
}
```

### Avoid Common Pitfalls

1. **Linear vs Planar**: Mode 13h is LINEAR — do not apply EGA plane logic
2. **Bounds**: 64000 bytes (not 8000 like EGA planes, not 32768 like old vram)
3. **Palette**: VGA DAC (256 entries), not 16-color EGA palette
4. **No drain Vec**: Write directly to vram (no `vga_writes: Vec` like the old plan suggested)
5. **Mode detection in bus.rs**: check `video.get_mode_type()` before routing A000 writes

## Implementation Order

1. **Expand vram + VideoMode variant** (Phase 1) — enum, vram resize, write/read/get helpers
2. **Bus routing** (Phase 2) — A000 reads/writes dispatch to VGA vs EGA path
3. **INT 10h** (Phase 3) — mode set, pixel I/O, text drawing
4. **Test programs** (Phase 6) — write basic tests before rendering
5. **Rendering** (Phase 4) — implement display after tests confirm buffer works
6. **Validation** (Phase 6) — test with real programs
7. **Documentation** (Phase 7)

## Success Criteria

- [ ] Test programs render correctly (colored bars, palette, text)
- [ ] At least one real DOS program (DOOM, Keen, etc.) displays graphics
- [ ] No crashes or memory corruption
- [ ] VGA DAC palette changes are visible
- [ ] Mode switching (text ↔ mode 13h) works cleanly
- [ ] Pre-commit checks pass (`./scripts/pre-commit.sh`)

## Future Enhancements

After basic mode 13h works:
- Double-buffering for smooth animation
- Dirty-rectangle tracking for partial updates
- Mode X support (tweaked mode 13h with planar access)
- VESA VBE modes (higher resolutions, more colors)