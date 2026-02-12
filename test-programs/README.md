
# Running

## .asm

1. Build

    ```bash
    nasm -f bin hello_program.asm -o hello_program.com
    ```

1. Run

    ```bash
    cargo run -p emu86-native-cli -- hello_program.com
    cargo run -p emu86-native-gui -- hello_program.com
    ```

## .bas

1. Create a disk image

    ```bash
    # Create a blank 1.44MB image
    mkfs.msdos -C audio.img 1440

    # Copy the file into the image
    mcopy -i audi.img -s audio.bas ::
    ```

1. Start the emulator running DOS
1. Load the img into a floppy drive
1. Open QBasic
1. Load program
1. Run

# Program Descriptions

## Audio

### audio.bas
QBasic program that plays a musical melody using the PLAY command. Demonstrates the BASIC PLAY statement for generating musical notes through the PC speaker.

### beep.asm
Assembly program that directly controls the PC speaker using the Intel 8253/8254 Programmable Interval Timer (PIT). Configures Channel 2 in Mode 3 (square wave), sets frequency to ~1000 Hz (count value 1193), enables the speaker via port 0x61, waits approximately 1 second, then disables the speaker and exits.

### simple_beep.asm
Simplified version of beep.asm that produces a 1000 Hz tone for approximately 1 second. Tests basic PC speaker functionality through PIT Channel 2 configuration and port 0x61 control.

## Keyboard

### waitkey.asm
Tests INT 16h AH=00h blocking keyboard input behavior. Waits for keypresses and echoes them back to the console. Demonstrates blocking keyboard reads, scan code/ASCII code handling, and prints each key pressed. Press ESC to exit.

## Misc

### hello_program.asm
Simple "Hello World" test program for verifying program loading functionality. Uses INT 21h AH=09h to display a message and INT 21h AH=4Ch to exit. Minimal .COM file structure starting at CS:0100h.

## Video

### mode_04h_cga_320x200x4.asm
Comprehensive CGA graphics mode test program. Switches to mode 0x04 (320x200, 4 colors) and tests both text rendering and graphics drawing in graphics mode.

**Expected Output:**
- **Pixel rows 0-39:** Three colored boxes at top (cyan, magenta, white) at columns 0, 20, 40
- **Pixel rows 48-63:** White text "CGA Graphics Mode 0x04 Test" (character row 6)
- **Pixel rows 64-71:** Magenta text "Drawing test patterns..." (character row 8)
- **Pixel rows 100-139:** Three colored boxes at middle (cyan, pattern, pattern) at columns 0, 20, 40
- **Pixel rows 184-191:** Cyan text "Test complete! Press any key..." at column 2 (character row 23)

**Technical Details:**
Demonstrates INT 10h AH=0Eh teletype output in graphics mode (draws 8x8 characters pixel-by-pixel using CP437 font), direct video memory writes at 0xB8000, pixel encoding (2 bits per pixel, 4 pixels per byte), CGA interlaced memory layout (even scan lines at 0x0000-0x1F3F, odd lines at 0x2000-0x3F3F), palette selection via port 0x3D9 (palette 1 with intensity: cyan, magenta, white), and proper cursor positioning in 40-column character grid (40 chars × 25 rows). Waits for keypress then returns to text mode.

### mode_04h_composite.asm
CGA composite mode test program. Starts in mode 0x04 (320x200, 4 colors), draws test patterns using 2bpp format, then enables composite mode by setting the hires bit (bit 4) in port 0x3D8. This creates a 640x200 composite artifact color mode as used by programs like Sierra AGI games (King's Quest I).

**Expected Output:**
- **Pixel rows 0-15:** 16 horizontal gradient bars, each showing a different nibble value (0x00 through 0xFF) rendered as composite colors
- **Pixel rows 100-139:** 8 vertical color bars using mixed nibble patterns (0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF)
- **Pixel rows 160-167:** Text "CGA Composite Mode Active!" (character row 20)
- **Pixel rows 184-191:** Text "Press any key to exit..." (character row 23)

**Technical Details:**
Demonstrates CGA composite mode activation via port 0x3D8 (changing from 0x0A to 0x1A to enable hires bit), nibble-to-palette rendering (each byte interpreted as 2 nibbles, each nibble 0-15 maps to a color), effective resolution of 160x200 pixels scaled to 640x400 display, and NTSC composite artifact coloring simulation. The program stays in mode 0x04 internally (CPU continues using 2bpp pixel format) but the renderer interprets the data differently when composite mode is active. Demonstrates how Sierra games and similar software achieved more colors through composite artifacts. Waits for keypress then returns to text mode.

### mode_06h_cga_640x200x2.asm
CGA graphics mode 0x06 test program. Switches to 640x200 monochrome mode (1 bit per pixel, 8 pixels per byte) and draws patterned boxes using direct video memory writes at 0xB800.

**Expected Output:**
- **Pixel rows 0-39:** Three patterned boxes at top (solid, dense checkerboard, alternating) at columns 0, 20, 40
- **Pixel rows 48-63:** White text "CGA Graphics Mode 0x06 Test" (character row 6)
- **Pixel rows 64-71:** White text "640x200, 2 Colors (1 bit/pixel)" (character row 8)
- **Pixel rows 100-139:** Three patterned boxes at middle (alternating, sparse, sparse checkerboard) at columns 0, 20, 40
- **Pixel rows 184-191:** White text "Test complete! Press any key..." at column 5 (character row 23)

**Technical Details:**
Demonstrates INT 10h AH=0Eh teletype output in 640x200 graphics mode, direct video memory writes, pixel encoding (1 bit per pixel, 8 pixels per byte), CGA interlaced memory layout, and 80-column character grid (80 chars × 25 rows). Waits for keypress then returns to text mode.

### mode_0dh_ega_320x200x16.asm
EGA graphics mode 0x0D test program. Switches to 320x200, 16-color mode using EGA planar memory at A000:0000. Displays all 16 EGA colors by programming the Sequencer Map Mask register (port 0x3C4/0x3C5) to select which bit planes receive writes.

**Expected Output:**
- **Pixel rows 0-39:** 8 colored boxes (colors 0-7): black, blue, green, cyan, red, magenta, brown, light gray — each 40 pixels wide spanning full screen width
- **Pixel rows 40-47:** Color number labels "0  1  2  3  4  5  6  7" (character row 5)
- **Pixel rows 56-63:** White text "EGA Mode 0x0D - 16 Colors" (character row 7)
- **Pixel rows 72-79:** Light cyan text "320x200, 4 Bit Planes" (character row 9)
- **Pixel rows 88-103:** Color name abbreviations for both rows (character rows 11, 13)
- **Pixel rows 120-159:** 8 colored boxes (colors 8-15): dark gray, light blue, light green, light cyan, light red, light magenta, yellow, white

**Technical Details:**
Demonstrates all 16 EGA colors via the planar memory model (4 bit planes at A000:0000, 40 bytes per row). Each color is drawn by clearing all planes then setting the Map Mask to the color value (each bit enables one plane). Shows EGA linear addressing (offset = row × 40 + column, no interlacing), INT 10h AH=0Eh teletype in 40-column grid, and labeled color swatches. Waits for keypress then returns to text mode.

### color.bas
QBasic program that demonstrates text mode color capabilities. Displays all 16 foreground colors (0-15) with black background, then shows standard background colors (0-7) with white foreground. Illustrates COLOR command usage and text mode color attribute handling.
