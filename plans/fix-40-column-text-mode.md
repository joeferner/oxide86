# Fix 40-Column Text Mode (Mode 0x01)

## Problem
Video mode 0x01 (40x25 text mode) displays incorrectly - screen goes black and cursor jumps around randomly.

## Root Causes
1. **Fixed buffer size**: Core video buffer is always 80x25, regardless of actual mode
2. **Memory addressing**: Programs write assuming 40 cells/row, but buffer stores 80 cells/row
3. **Rendering**: GUI always renders all 80 columns, showing garbage in cols 40-79
4. **Cursor calculation**: Always divides by 80 instead of actual column count

## Solution

### 1. Update core/src/video.rs
- Store actual mode dimensions in `Video` struct
- Fix `read_byte`/`write_byte` to account for actual column width when indexing buffer
- Fix cursor calculations in `VgaIoPorts::write_data()` to use actual columns
- When in 40-column mode, map memory addresses appropriately

### 2. Update native-gui/src/gui_video.rs
- Modify `render()` to loop through actual mode dimensions, not hardcoded TEXT_MODE_COLS
- Fix cell indexing to use actual column count
- Handle 40-column mode by rendering only left 40 columns (or scale up)

### 3. Similar fixes for other platforms
- wasm/src/web_video.rs (if exists)
- native-cli terminal video

## Implementation Approach
Option A: Keep 80x25 buffer, but map 40-column addresses correctly
- Simpler for rendering (always 80x25)
- Requires address translation in read/write

Option B: Dynamic buffer sizing based on mode
- More complex, requires changing buffer to Vec or enum
- More accurate to actual hardware

**Recommendation**: Use Option A for simplicity - the buffer stays 80x25 internally, but memory access and rendering respect the actual mode dimensions.

## Testing
- Run Checkit graphics test and verify mode 0x01 displays correctly
- Verify mode 0x00 (40x25 B&W) also works
- Verify modes 0x02, 0x03 (80x25) still work correctly
