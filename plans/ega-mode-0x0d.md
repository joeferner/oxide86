# EGA Mode 0x0D Implementation Plan

## Overview
Mode 0x0D: EGA 320x200, 16 colors, 4 bit planes
- Video memory at A000:0000 (64KB, not B800)
- 4 planes, each 8000 bytes (320*200/8)
- Pixel color = 4 bits (1 bit from each plane at same address/bit position)

## EGA I/O Registers (minimal set)
- **Sequencer index** port 0x3C4, data port 0x3C5
  - Register 2: Map Mask (which planes receive writes), default 0x0F
- **Graphics Controller index** port 0x3CE, data port 0x3CF
  - Register 4: Read Map Select (which plane to read), default 0
  - Register 5: Mode register (write/read mode), default 0
  - Register 8: Bit Mask, default 0xFF

## Files to Change

### 1. core/src/video.rs
- Add `Graphics320x200x16` to `VideoMode` enum
- Add `EgaBuffer` struct: 4 planes, each 8000 bytes
- Add EGA register state to `Video` struct:
  - `ega_sequencer_map_mask: u8` (default 0x0F)
  - `ega_read_plane: u8` (default 0)
- Add `ega_buffer: Option<EgaBuffer>` to `Video`
- `EgaBuffer::write_byte(plane_mask, offset, value)` - write to enabled planes
- `EgaBuffer::read_byte(plane, offset)` - read from selected plane
- `EgaBuffer::get_pixels()` - compose 16-color pixel array (320*200 bytes, 0-15)
- Update `set_mode()` for 0x0D: allocate EgaBuffer, clear text/graphics buffers
- Add `write_byte_ega(offset, value)` - writes to selected planes via map_mask
- Add `read_byte_ega(offset) -> u8` - reads from selected read plane
- Add `get_ega_buffer()` getter
- Update `get_cols()`, `get_rows()` for mode 0x0D (40x25 text cursor space)
- Add `set_ega_map_mask()`, `set_ega_read_plane()` setters

### 2. core/src/memory.rs
- Add `EGA_MEMORY_START: usize = 0xA0000`, `EGA_MEMORY_END: usize = 0xAFFFF`
- Add `ega_writes: Vec<(usize, u8)>` to `Memory`
- In `write_u8()`: detect A0000-AFFFF range, push to `ega_writes`
- Add `drain_ega_writes()` method

### 3. core/src/io/mod.rs
- Add EGA I/O state to `IoDevice`:
  - `ega_sequencer_index: u8`
  - `ega_graphics_index: u8`
- Handle ports 0x3C4 (seq index), 0x3C5 (seq data), 0x3CE (gc index), 0x3CF (gc data)
- On write to 0x3C5 with index=2: call `video.set_ega_map_mask(value)`
- On write to 0x3CF with index=4: call `video.set_ega_read_plane(value & 3)`
- On read from 0x3C5/0x3CF: return stored register values

### 4. core/src/computer.rs
- In `update_video()`: add `VideoMode::Graphics320x200x16` rendering path
  - Drain ega_writes, call `video.write_byte_ega(offset, value)`
  - Call `video_controller.update_graphics_320x200x16(pixels)`
- In `force_video_redraw()`: add matching path
- Add `VideoMode::Graphics320x200x16` to the mode sync check

### 5. core/src/cpu/bios/int10.rs
- In `int10_set_video_mode()`: extend match to include 0x0D
  - Call `video.set_mode(mode)`, reset cursor, update BDA (40 cols for 0x0D)
  - Note: mode 0x0D is 320x200 graphics, BDA cols = 40, page_size irrelevant

### 6. core/src/video.rs VideoController trait
- Add `update_graphics_320x200x16(&mut self, pixel_data: &[u8])` with default no-op

### 7. native-gui/src/gui_video.rs
- Implement `update_graphics_320x200x16()`: use VGA DAC palette, 320x200 -> window scale

### 8. wasm/src/web_video.rs
- Implement `update_graphics_320x200x16()`: render to canvas

## Implementation Notes
- EgaBuffer pixel composition: for each (x,y) at byte offset `o = y*40 + x/8`, bit `b = 7-(x%8)`:
  `color = (plane0[o]>>b&1) | ((plane1[o]>>b&1)<<1) | ((plane2[o]>>b&1)<<2) | ((plane3[o]>>b&1)<<3)`
- Write Mode 0 (simplest, default): CPU byte written to all enabled planes
- For is_graphics_mode() checks: include mode 0x0D
