# CGA Composite Emulation for 640x200 Mode

## Context

King's Quest 1 (Sierra AGI engine) uses CGA composite artifact coloring:
1. Sets mode 0x04 (320x200 CGA) via INT 10h
2. Writes 0x1A to port 0x3D8, switching to 640x200 1bpp mode
3. Draws pixel patterns designed to produce colors on a composite CGA monitor

On real composite monitors, groups of 4 adjacent 1bpp pixels create NTSC artifact colors, producing ~160x200 effective resolution with 16 colors. Our emulator renders this as B&W (RGB monitor behavior), causing:
- **Black and white screen** instead of pink/cyan/white/black
- **Small font text** because 640-pixel-wide characters appear tiny without 4:1 composite grouping

## Approach

**Simple nibble-to-palette composite rendering** in both renderers (native-gui, wasm):
- Each byte (8 pixels) splits into 2 nibbles (4 pixels each)
- Each nibble value (0-15) maps to the standard 16-color CGA palette via `TextModePalette::get_color()`
- Effective output: 160x200 composite pixels, scaled to 640x400 (4x horizontal, 2x vertical)

This works because IBM designed CGA so that aligned 4-pixel patterns produce artifact colors matching the standard CGA palette.

## Files to Modify

### 1. `native-gui/src/gui_video.rs` - `render_graphics_640x200()` (lines 199-232)

Replace the per-pixel B&W rendering loop with composite nibble rendering:

```rust
fn render_graphics_640x200(&self, frame: &mut [u8]) {
    if let Some(pixel_data) = &self.graphics_data {
        let scale_y = 2;  // 200 → 400
        let scale_x = 4;  // 160 → 640

        for y in 0..200 {
            for byte_x in 0..80 {
                let byte_val = pixel_data[y * 80 + byte_x];
                let high_nibble = (byte_val >> 4) & 0x0F;
                let low_nibble = byte_val & 0x0F;

                // Left composite pixel (from high nibble)
                let color_left = TextModePalette::get_color(high_nibble);
                // Right composite pixel (from low nibble)
                let color_right = TextModePalette::get_color(low_nibble);

                for (i, rgb) in [(0, color_left), (1, color_right)] {
                    let composite_x = byte_x * 2 + i;
                    for dy in 0..scale_y {
                        for dx in 0..scale_x {
                            let screen_x = composite_x * scale_x + dx;
                            let screen_y = y * scale_y + dy;
                            let offset = (screen_y * SCREEN_WIDTH + screen_x) * 4;
                            frame[offset] = rgb[0];
                            frame[offset + 1] = rgb[1];
                            frame[offset + 2] = rgb[2];
                            frame[offset + 3] = 0xFF;
                        }
                    }
                }
            }
        }
    }
}
```

### 2. `wasm/src/web_video.rs` - `render_graphics_640x200()` (around line 232)

Same composite nibble rendering logic, adapted for the WASM canvas context.

### 3. No core changes needed

The data pipeline (CgaBuffer → get_pixels() → update_graphics_640x200()) stays the same. The renderers just interpret the 1bpp data differently.

## Verification

1. Run KQ1 CGA: `cargo run -p emu86-native-gui -- --boot --floppy-a /home/fernejo/dos/kq1-cga.img`
2. Verify colors appear (pink/magenta, cyan, white, black)
3. Verify text at the bottom is readable at normal size
4. Run `./scripts/pre-commit.sh` for build/lint
