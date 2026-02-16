# INT 10h AH=0Eh Teletype Output - Attribute Preservation

## Summary

INT 10h AH=0Eh (teletype output) in text mode **preserves existing character attributes** at the cursor position, with one exception: attribute 0x00 (black on black) is replaced with 0x07 (light gray on black) since invisible text is never useful.

## Real BIOS Behavior

According to [IBM PC BIOS documentation](http://vitaly_filatov.tripod.com/ng/asm/asm_023.15.html):

> In text modes, the character displayed retains the display attribute of the previous character that occupied the screen location.

This means teletype output:
- **Text mode**: Writes the character byte only, preserves the existing attribute byte
- **Graphics mode**: Uses BL register for foreground color

## The Problem

Programs like MS-DOS EDIT and Checkit leave the screen with attribute 0x00 (black on black) when they exit:

1. Program runs with its own colored interface (e.g., blue for EDIT)
2. On exit, the program clears the screen by writing spaces with attribute 0x00
3. The program does **not** call INT 10h AH=00h (set video mode) to reset the screen
4. COMMAND.COM displays the DOS prompt using INT 21h → INT 29h → INT 10h AH=0Eh
5. Teletype preserves the existing 0x00 attribute
6. Result: DOS prompt is written as black text on black background (invisible)

### Programs Affected
- **MS-DOS EDIT**: Writes 0x00 to port 0x3D9, sets cursor position, but never resets video mode
- **Checkit**: Clears screen with attribute 0x00 before exiting

### Why "dir" Would Fix It (Before Our Fix)

When you type "dir" and press Enter:
1. DIR command calls INT 10h AH=06h (scroll up) with attribute 0x07
2. This fills the screen with spaces using attribute 0x07 (light gray on black)
3. Subsequent text output preserves the new 0x07 attribute
4. Text becomes visible

## Fix 1: Teletype 0x00 Attribute Substitution

**File**: `core/src/cpu/bios/int10.rs` - `int10_teletype_output()`

In the teletype handler, after writing the character byte, if the existing attribute is 0x00 (black on black), we substitute 0x07 (light gray on black):

```rust
video.write_byte(offset, ch);
if existing_attr == 0x00 {
    video.write_byte(offset + 1, 0x07);
}
```

Rationale:
- Attribute 0x00 (black on black) is never useful - text is always invisible
- Many BIOS implementations (Phoenix, AMI) have similar compatibility measures
- Only affects the specific case of 0x00; all other attributes are preserved accurately

## Fix 2: VGA DAC Palette Corruption from Port 0x3D9 in Text Mode

**File**: `core/src/video/mod.rs` - `set_palette()`, `set_cga_background()`, `set_cga_intensity()`, `set_cga_palette_id()`

### The Problem

When EDIT exits, it writes 0x00 to port 0x3D9 (CGA Color Select Register). This triggered `update_vga_dac_from_cga_palette()`, which overwrites VGA DAC entries 0-3 with CGA palette 0 colors:

| VGA DAC Entry | Expected (default) | After CGA sync (palette 0) |
|---|---|---|
| 0 | Black | Black |
| 1 | **Blue** | **Green** |
| 2 | Green | Red |
| 3 | Cyan | Brown |

When EDIT (or any program) next uses color index 1 for a blue background, it renders as **green** instead.

### The Fix

`update_vga_dac_from_cga_palette()` is now only called when in CGA graphics mode (modes 0x04-0x06). In text mode, port 0x3D9 writes only affect the CGA palette state (border/overscan) without corrupting the VGA DAC text color palette:

```rust
pub fn set_palette(&mut self, value: u8) {
    self.palette = CgaPalette::from_register(value);
    if self.is_cga_graphics_mode() {
        self.update_vga_dac_from_cga_palette();
    }
    self.dirty = true;
}
```

## Related Information

- **VGA DAC Palette**: Programs can modify the VGA DAC palette (INT 10h AH=10h AL=10h). Video mode changes (INT 10h AH=00h) reset the palette to defaults.
- **CGA Palette Register**: Port 0x3D9 controls CGA palette/background. Only affects VGA DAC in CGA graphics modes (0x04-0x06).
- **Text Attributes**: Attribute byte format is `[blink][bg2][bg1][bg0][fg3][fg2][fg1][fg0]`
  - Bits 0-3: Foreground color (0-15)
  - Bits 4-6: Background color (0-7)
  - Bit 7: Blink enable

## References

- [INT 10h Teletype Output Reference](http://vitaly_filatov.tripod.com/ng/asm/asm_023.15.html)
- [BIOS Interrupt Call Documentation](https://en.wikipedia.org/wiki/BIOS_interrupt_call)
- [INT 10H Reference](http://employees.oneonta.edu/higgindm/assembly/DOS_AND_ROM_BIOS_INTS.htm)
- [DOS INT 21h AH=4Ch](https://stanislavs.org/helppc/int_21-4c.html)

## Investigation Dates

- February 7, 2026 - Investigated invisible text after Checkit exit, confirmed real BIOS behavior
- February 16, 2026 - Fixed invisible prompt after EDIT/Checkit exit (0x00→0x07 substitution) and green screen bug (VGA DAC corruption from port 0x3D9 in text mode)
