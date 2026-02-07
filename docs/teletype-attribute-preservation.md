# INT 10h AH=0Eh Teletype Output - Attribute Preservation

## Summary

INT 10h AH=0Eh (teletype output) in text mode **preserves existing character attributes** at the cursor position. This is accurate real BIOS behavior, but can cause invisible text if programs leave the screen with attribute 0x00 (black on black).

## Real BIOS Behavior

According to [IBM PC BIOS documentation](http://vitaly_filatov.tripod.com/ng/asm/asm_023.15.html):

> In text modes, the character displayed retains the display attribute of the previous character that occupied the screen location.

This means teletype output:
- **Text mode**: Writes the character byte only, preserves the existing attribute byte
- **Graphics mode**: Uses BL register for foreground color

## The Checkit Problem

### What Happens

1. Checkit diagnostic program runs with its own blue interface
2. On exit, checkit clears the screen by writing spaces with attribute 0x00 (black on black)
3. COMMAND.COM displays the DOS prompt using INT 21h console output
4. INT 21h uses INT 10h teletype internally
5. Teletype preserves the existing 0x00 attribute
6. Result: DOS prompt is written as black text on black background (invisible)

### Log Evidence

```
[09:59:20.917 INFO] INT 10h AH=0Eh: Writing 'C' (0x43) at (17,0) - existing attr=0x00 (fg=0, bg=0)
[09:59:20.917 INFO] INT 10h AH=0Eh: Writing ':' (0x3A) at (17,1) - existing attr=0x00 (fg=0, bg=0)
[09:59:20.917 INFO] INT 10h AH=0Eh: Writing '\' (0x5C) at (17,2) - existing attr=0x00 (fg=0, bg=0)
...
```

Every character of "C:\CHECKIT2>" is written with black on black attributes.

### Why "dir" Fixes It

When you type "dir" and press Enter:
1. DIR command calls INT 10h AH=06h (scroll up) with attribute 0x07
2. This fills the screen with spaces using attribute 0x07 (light gray on black)
3. Subsequent text output preserves the new 0x07 attribute
4. Text becomes visible

## Why Real PCs Don't Have This Issue

There are several possibilities:

1. **Checkit restores properly on real hardware**: The real checkit might detect real hardware and restore video state correctly
2. **DOS version differences**: Different DOS versions might handle screen initialization differently
3. **BIOS variations**: Real PC BIOS might have vendor-specific variations in teletype behavior
4. **TSR behavior**: Real systems might have TSRs that intercept video operations

According to [DOS INT 21h AH=4Ch documentation](https://stanislavs.org/helppc/int_21-4c.html), DOS does **NOT** automatically restore video mode or screen attributes when programs exit.

## Design Decision

We chose to **keep the accurate BIOS behavior** for several reasons:

1. **Accuracy**: Emulating real BIOS behavior is more important than working around buggy programs
2. **Compatibility**: Programs that work correctly on real hardware will work correctly here
3. **Debugging**: Accurate behavior helps identify when programs have bugs
4. **Standards**: Following documented BIOS behavior ensures long-term compatibility

## Implications for DOS Programs

Programs that modify video state (mode, palette, or attributes) should:

1. **Save video state on startup**:
   ```asm
   ; Get current video mode
   mov ah, 0Fh
   int 10h
   ; AL = current mode, save it
   ```

2. **Restore video state on exit**:
   ```asm
   ; Restore original video mode
   mov ah, 00h
   mov al, [saved_mode]
   int 10h
   ```

3. **Or clear screen with proper attributes**:
   ```asm
   ; Clear screen with light gray on black (0x07)
   mov ax, 0600h     ; AH=06h (scroll up), AL=00h (clear)
   mov bh, 07h       ; Attribute: light gray on black
   mov cx, 0000h     ; Top-left (0,0)
   mov dx, 184Fh     ; Bottom-right (24,79)
   int 10h
   ```

## Related Information

- **VGA DAC Palette**: Programs can also modify the VGA DAC palette (INT 10h AH=10h AL=10h). Video mode changes reset the palette to defaults.
- **CGA Palette Register**: Port 0x3D9 controls CGA palette/background, mainly for graphics modes
- **Text Attributes**: Attribute byte format is `[blink][bg2][bg1][bg0][fg3][fg2][fg1][fg0]`
  - Bits 0-3: Foreground color (0-15)
  - Bits 4-6: Background color (0-7)
  - Bit 7: Blink enable

## References

- [INT 10h Teletype Output Reference](http://vitaly_filatov.tripod.com/ng/asm/asm_023.15.html)
- [BIOS Interrupt Call Documentation](https://en.wikipedia.org/wiki/BIOS_interrupt_call)
- [INT 10H Reference](http://employees.oneonta.edu/higgindm/assembly/DOS_AND_ROM_BIOS_INTS.htm)
- [DOS INT 21h AH=4Ch](https://stanislavs.org/helppc/int_21-4c.html)
- [DOS Function Calls](https://philadelphia.edu.jo/academics/qhamarsheh/uploads/Lecture%2021%20MS-DOS%20Function%20Calls%20_INT%2021h_.pdf)

## Investigation Date

February 7, 2026 - Investigated invisible text after checkit exit, confirmed real BIOS behavior via logging and documentation research.
