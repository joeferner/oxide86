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

## Implementation Plan

### Phase 1: Core Video Infrastructure

**File: `core/src/video/mod.rs`**

1. Add new enum variant to `VideoMode`:
   ```rust
   /// VGA 320x200, 256 colors (mode 0x13)
   Graphics320x200x256,
   ```

2. Create new buffer type in `core/src/video/vga.rs`:
   ```rust
   /// VGA linear framebuffer for mode 0x13 (320x200 256-color)
   /// Direct pixel access: each byte is a 0-255 color index
   pub struct VgaBuffer {
       pixels: [u8; 64000],  // 320 × 200
   }

   impl VgaBuffer {
       pub fn new() -> Self { ... }
       pub fn write_byte(&mut self, offset: usize, value: u8) { ... }
       pub fn read_byte(&self, offset: usize) -> u8 { ... }
       pub fn get_pixels(&self) -> &[u8] { ... }  // For rendering

       // Scrolling support for INT 10h AH=06h/07h
       pub fn scroll_up_window(&mut self, lines, top, left, bottom, right) { ... }
       pub fn scroll_down_window(&mut self, lines, top, left, bottom, right) { ... }
   }
   ```

3. Add `VgaBuffer` field to `Video` struct:
   ```rust
   pub struct Video {
       // ... existing fields ...
       vga_buffer: VgaBuffer,
   }
   ```

4. Update `Video::set_mode()` to handle mode 0x13:
   ```rust
   0x13 => {
       self.mode = 0x13;
       self.mode_type = VideoMode::Graphics320x200x256;
       self.vga_buffer = VgaBuffer::new();
       // ... BDA updates, palette reset ...
   }
   ```

5. Update mode helpers (`get_cols()`, `get_rows()`, `is_graphics_mode()`):
   - `get_cols()`: return 40 for mode 0x13 (40 text columns @ 8px/char)
   - `get_rows()`: return 25 for mode 0x13
   - Include `Graphics320x200x256` in graphics mode checks

### Phase 2: Memory System Integration

**File: `core/src/memory.rs`**

1. Track VGA writes alongside EGA writes:
   ```rust
   pub struct Memory {
       // ... existing fields ...
       vga_writes: Vec<(usize, u8)>,  // (offset, value) for A000 writes in mode 13h
   }
   ```

2. Update `write_byte()` to capture mode 13h writes:
   ```rust
   // In A000:0000 - A000:FFFF range
   if video_mode == 0x13 {
       self.vga_writes.push((offset_in_segment, value));
   } else if video_mode == 0x0D {
       self.ega_writes.push((offset_in_segment, value));
   }
   ```

3. Add drain method:
   ```rust
   pub fn drain_vga_writes(&mut self) -> Vec<(usize, u8)> {
       std::mem::take(&mut self.vga_writes)
   }
   ```

**File: `core/src/bus.rs`**

4. Add VgaBuffer accessor:
   ```rust
   pub fn get_vga_buffer(&self) -> Option<&VgaBuffer> {
       if matches!(self.video.get_mode_type(), VideoMode::Graphics320x200x256) {
           Some(&self.video.vga_buffer)
       } else {
           None
       }
   }

   pub fn get_vga_buffer_mut(&mut self) -> Option<&mut VgaBuffer> { ... }
   ```

5. Update `apply_pending_writes()` or create `apply_vga_writes()`:
   ```rust
   pub fn apply_vga_writes(&mut self) {
       let writes = self.memory.drain_vga_writes();
       if let Some(buffer) = self.get_vga_buffer_mut() {
           for (offset, value) in writes {
               buffer.write_byte(offset, value);
           }
       }
   }
   ```

### Phase 3: INT 10h Video BIOS

**File: `core/src/cpu/bios/int10.rs`**

1. Add mode 0x13 to supported modes list:
   ```rust
   fn int10_set_mode(&mut self, bus: &mut Bus) {
       match mode {
           0x00..=0x07 | 0x0D | 0x13 => {  // Added 0x13
               bus.video_mut().set_composite_mode(false);
               bus.video_mut().set_mode(mode, false);
               // ... BDA updates ...
           }
           _ => {
               log::warn!("INT 10h AH=00h: Unsupported mode 0x{:02X}", mode);
           }
       }
   }
   ```

2. Update pixel read/write functions:

   **AH=0Ch (Write Pixel)**:
   ```rust
   VideoMode::Graphics320x200x256 => {
       if x < 320 && y < 200 {
           let offset = y * 320 + x;
           if let Some(buffer) = bus.get_vga_buffer_mut() {
               buffer.write_byte(offset, color as u8);
           }
       }
   }
   ```

   **AH=0Dh (Read Pixel)**:
   ```rust
   VideoMode::Graphics320x200x256 => {
       if x < 320 && y < 200 {
           let offset = y * 320 + x;
           if let Some(buffer) = bus.get_vga_buffer() {
               self.cpu.ax = buffer.read_byte(offset) as u16;
           }
       }
   }
   ```

3. Update text drawing in graphics mode:
   - `draw_char_graphics()` needs to support mode 13h
   - Same pixel coordinate system (320x200)
   - Use VGA DAC palette for foreground color
   - Affects: AH=09h, AH=0Ah, AH=0Eh, AH=13h

4. Update scrolling (AH=06h/07h):
   ```rust
   VideoMode::Graphics320x200x256 => {
       if let Some(buffer) = bus.get_vga_buffer_mut() {
           buffer.scroll_up_window(lines, top, left, bottom, right);
       }
   }
   ```

### Phase 4: Rendering Pipeline

**File: `core/src/video/mod.rs` (VideoController trait)**

1. Add new trait method:
   ```rust
   /// Update VGA graphics display (320x200, 256 colors, mode 0x13)
   /// pixel_data: linear pixel array (320*200 bytes), each byte is a 0-255 color index
   /// Uses VGA DAC palette for RGB conversion
   fn update_graphics_320x200x256(&mut self, pixel_data: &[u8]) {
       let _ = pixel_data;
       log::warn!("Graphics mode 320x200x256 (VGA mode 13h) not implemented for this platform");
   }
   ```

**File: `core/src/computer.rs`**

2. Update `update_video()`:
   ```rust
   match mode_type {
       // ... existing cases ...
       VideoMode::Graphics320x200x256 => {
           // Apply VGA writes to buffer
           self.bus.apply_vga_writes();

           // Render
           if let Some(buffer) = self.bus.get_vga_buffer() {
               self.video_controller.update_graphics_320x200x256(buffer.get_pixels());
           }
       }
   }
   ```

3. Update `force_redraw()` similarly

**File: `native-gui/src/gui_video.rs`**

4. Implement rendering for native GUI:
   ```rust
   fn update_graphics_320x200x256(&mut self, pixel_data: &[u8]) {
       self.graphics_data_256 = Some(pixel_data.to_vec());
       self.has_pending_updates = true;
   }

   // In render() method:
   if let Some(ref pixels_256) = self.graphics_data_256 {
       self.render_graphics_320x200x256(pixels_256, window)?;
   }
   ```

5. Add render method:
   ```rust
   fn render_graphics_320x200x256(&mut self, pixels: &[u8], window: &Window) -> Result<()> {
       // Scale 320x200 → 640x400 (2x2) or larger
       // For each pixel: color_index = pixels[y*320 + x]
       // RGB = vga_dac_to_rgb(self.vga_dac_palette[color_index])
       // Draw scaled pixel
   }
   ```

**File: `wasm/src/web_video.rs`**

6. Implement for WASM:
   ```rust
   fn update_graphics_320x200x256(&mut self, pixel_data: &[u8]) {
       if let Err(e) = self.render_graphics_320x200x256(pixel_data) {
           log::error!("Failed to render 320x200x256 VGA graphics: {:?}", e);
       }
   }

   fn render_graphics_320x200x256(&self, pixels: &[u8]) -> Result<()> {
       // Get canvas 2D context
       // Create ImageData (scale appropriately)
       // Map pixel indices through VGA DAC palette
       // putImageData()
   }
   ```

### Phase 5: I/O Port Handling (if needed)

**File: `core/src/io/mod.rs`**

Mode 13h uses standard VGA registers already implemented:
- Sequencer (0x3C4/0x3C5): Map Mask (though not critical for linear mode)
- VGA DAC (0x3C7/0x3C8/0x3C9): Palette read/write (already implemented)
- No new I/O ports needed

**Verify existing VGA DAC implementation supports 256 entries** (currently should have `[[u8; 3]; 256]`)

### Phase 6: Testing & Validation

**Test Programs (create in `test-programs/video/`)**:

1. **mode13h-test.asm** - Basic mode test:
   ```asm
   ; Set mode 13h
   mov ax, 0013h
   int 10h

   ; Draw colored bars
   mov ax, 0A000h
   mov es, ax
   xor di, di
   mov cx, 320*200

   draw_loop:
       mov al, cl  ; Color index = position % 256
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

2. **mode13h-palette.asm** - Test VGA DAC:
   ```asm
   ; Set mode 13h
   ; Set up custom palette (greyscale ramp)
   ; Draw gradient
   ; Test INT 10h AH=0Ch/0Dh pixel read/write
   ```

3. **mode13h-text.asm** - Test text drawing in mode 13h:
   ```asm
   ; Set mode 13h
   ; Use INT 10h AH=09h/0Eh to draw text
   ; Verify XOR mode works
   ```

**Real-world test programs**:
- **DOOM**: Famous for using mode 13h
- **Commander Keen 4-6**: Use mode 13h
- **Duke Nukem**: Uses mode 13h
- **PC Paint**: Drawing program using mode 13h

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

### Avoid Common Pitfalls

1. **Linear vs Planar**: Mode 13h is LINEAR - do not apply EGA plane logic
2. **Bounds checking**: 64000 bytes, not 8000 like EGA planes
3. **Palette**: Use VGA DAC palette (256 entries), not 16-color EGA palette
4. **Memory efficiency**: 64KB buffer per instance - consider if cloning is needed
5. **Mode detection**: Ensure memory.rs distinguishes mode 0x13 from 0x0D for A000 writes

### Performance Considerations

- Mode 13h buffer is 64KB (larger than EGA's 32KB total)
- Rendering may need double-buffering to avoid flicker
- Consider dirty-rectangle tracking if many small updates
- Palette lookups happen per-pixel - cache RGB conversions if needed

### Hardware Compatibility

- Mode 13h introduced with VGA (1987)
- Not available on CGA/EGA systems
- Programs should check for VGA before setting mode 13h
- Some programs query INT 10h AH=1Ah (display combination code) first

## Implementation Order

Recommended order to minimize integration issues:

1. **Video infrastructure** (Phase 1) - Add enum, buffer type
2. **Memory system** (Phase 2) - Track writes to A000
3. **INT 10h** (Phase 3) - Mode set, pixel I/O
4. **Test programs** (Phase 6) - Write basic tests BEFORE rendering
5. **Rendering** (Phase 4) - Implement display after tests confirm buffer works
6. **Validation** (Phase 6) - Test with real programs
7. **Documentation** (Phase 7) - Update docs

This order allows testing buffer logic independently of rendering.

## Estimated Complexity

- **Low-Medium complexity** - Mode 13h is simpler than EGA mode 0x0D
- **Linear memory** makes implementation straightforward
- Main work is boilerplate: adding new enum case to all match statements
- Rendering is nearly identical to mode 0x0D, just 256 colors instead of 16

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