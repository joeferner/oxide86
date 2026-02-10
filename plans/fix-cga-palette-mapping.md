# Fix CGA 320x200 Palette Mapping

## Problem

When programs set custom VGA DAC colors for mode 0x04 (320x200 4-color), the wrong colors are displayed.

**Root Cause:**
- CGA pixel values 0-3 should map directly to VGA DAC palette entries 0-3
- Currently: `CgaPalette::get_colors()` returns `[0, 3, 5, 15]` (EGA color indices)
- Rendering uses these as VGA DAC indices: pixel 3 → EGA index 15 → VGA_DAC[15]
- But programs like checkit only set VGA_DAC[0-3], not VGA_DAC[15]!

**Why Alleycat works:**
- Alleycat doesn't set custom VGA DAC colors
- Default VGA DAC has standard EGA colors at indices 0, 3, 5, 15
- So the indirect mapping happens to work

## Solution

### Part 1: Update VGA DAC when CGA palette changes

When INT 10h AH=0Bh changes the CGA palette, update VGA DAC entries 0-3:

**File:** `core/src/video.rs`
- Modify `set_cga_background()` to also call `update_vga_dac_from_cga_palette()`
- Modify `set_cga_palette_id()` to also call `update_vga_dac_from_cga_palette()`
- Modify `set_cga_intensity()` to also call `update_vga_dac_from_cga_palette()`
- Add helper method `update_vga_dac_from_cga_palette()` that:
  - Gets the 4 colors from `self.palette.get_colors()` (EGA indices)
  - For each color, sets VGA DAC[i] to the default RGB for that EGA color
  - Example: If palette colors are [0, 3, 5, 15], set:
    - VGA_DAC[0] = default_vga_palette()[0] (black)
    - VGA_DAC[1] = default_vga_palette()[3] (cyan)
    - VGA_DAC[2] = default_vga_palette()[5] (magenta)
    - VGA_DAC[3] = default_vga_palette()[15] (white)

### Part 2: Simplify CGA 320x200 rendering

Change rendering to use VGA DAC indices 0-3 directly:

**File:** `native-gui/src/gui_video.rs` - `render_graphics_320x200()`
- Line 171: `let color_index = ((byte_val >> shift) & 0x03) as usize;`
- Line 174-175: Currently uses `palette[color_index]` as VGA DAC index
- **Change to:** Use `color_index` directly as VGA DAC index
- Remove the `palette` parameter/field for 320x200 mode

**File:** `native-gui/src/gui_video.rs` - `update_graphics_320x200()`
- Remove the `palette` parameter
- Remove line 425-426 that stores palette colors

**File:** `native-gui/src/gui_video.rs` - struct fields
- Remove `graphics_palette: Option<[u8; 4]>` field (no longer needed)

**File:** `core/src/video.rs` - VideoController trait
- Update `update_graphics_320x200()` signature to not take palette parameter

**File:** `core/src/computer.rs` - `update_video()`
- Update call to `update_graphics_320x200()` to not pass palette

### Part 3: Also update WASM rendering

**File:** `wasm/src/web_video.rs`
- Apply same changes as native-gui

## Testing

- Checkit graphics test should show white text on black (not blue)
- Alleycat should still work correctly
- Programs using INT 10h AH=0Bh to change palettes should update colors properly

## Files to Modify

1. `core/src/video.rs` - Add VGA DAC update logic
2. `native-gui/src/gui_video.rs` - Simplify rendering
3. `wasm/src/web_video.rs` - Simplify rendering
4. `core/src/computer.rs` - Update VideoController calls
